use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::vec::Vec;
use std::path::{Path, PathBuf};
use std::marker::Send;
use std::env;
use tempfile::NamedTempFile;
use std::io::Write;
use std::process::Command;
use itertools::Itertools;
use std::thread;

use crate::{Place, PlaceMaker, Edge};


pub struct Maker {
    places: HashMap<String, PlaceMaker>,
    edges: HashMap<String, Edge>,
    places_to_edges: HashMap<String, HashSet<String>>,
    edges_to_places: HashMap<String, HashSet<String>>,

}
impl Maker {
    pub fn make() -> Self {
        Self {
            places: HashMap::new(),
            edges: HashMap::new(),
            places_to_edges: HashMap::new(),
            edges_to_places: HashMap::new(),
        }
    }
    pub fn add_place(mut self, name: &str, p: PlaceMaker) -> Self {
        self.places.insert(name.into(), p);
        self.places_to_edges.insert(name.into(), HashSet::new());
        self
    }
    pub fn add_edge<T: 'static + Send>(mut self, name: &str) -> Self {
        self.edges.insert(name.into(), Edge {
                _name: name.into(),
                type_name: type_name::<T>().into(),
                type_id: TypeId::of::<T>(),
                vec: VecDeque::new(),
        });
        self.edges_to_places.insert(name.into(), HashSet::new());
        self
    }
    pub fn set_start_tokens<T: 'static + Send>(mut self, edge: &str, mut start_tokens: Vec<T>) -> Self {
        match self.edges.get_mut(&edge.to_string()) {
            Some(e) => {
                for t in start_tokens.drain(..) {
                    e.push(Box::new(t));
                }
            }
            None => {
                self = self.add_edge::<T>(edge.into()).set_start_tokens::<T>(edge, start_tokens);
            }
        }
        self
    }
    pub fn place_to_edge(mut self, place: &str, edge: &str) -> Self {
        match self.places_to_edges.get_mut(&place.to_string()) {
            Some(s) => {
                s.insert(edge.into());
            },
            None => {
                self.places_to_edges.insert(place.into(), HashSet::new());
                self.places_to_edges.get_mut(&place.to_string()).unwrap().insert(edge.into());
            }
        };
        self
    }
    pub fn edge_to_place(mut self, edge: &str, place: &str) -> Self {
        match self.edges_to_places.get_mut(&edge.to_string()) {
            Some(s) => {
                s.insert(place.into());
            },
            None => {
                self.edges_to_places.insert(edge.into(), HashSet::new());
                self.edges_to_places.get_mut(&edge.to_string()).unwrap().insert(place.into());
            }
        };
        self
    }
    fn pseudo_hash(&self) -> u64 {
        let mut places = self.places.keys().collect::<Vec<_>>();
        places.sort();
        let mut edges = self.edges.keys().collect::<Vec<_>>();
        edges.sort();
        let mut places_to_edges = vec![];
        for (p, es) in self.places_to_edges.iter().sorted_by_key(|x| x.0) {
            let mut e = es.iter().collect::<Vec<_>>();
            e.sort();
            places_to_edges.push((p, e));
        }
        let mut edges_to_places = vec![];
        for (e, ps) in self.edges_to_places.iter().sorted_by_key(|x| x.0) {
            let mut p = ps.iter().collect::<Vec<_>>();
            p.sort();
            edges_to_places.push((e, p));
        }
        let mut s = DefaultHasher::new();
        let t = (places, edges, places_to_edges, edges_to_places);
        t.hash(&mut s);
        s.finish()
    }
    pub fn png(&self) -> PathBuf {
        let graph_cache = env::current_exe().expect("Getting current exe")
            .as_path().parent().unwrap()
            .join(Path::new("graph_png_cache"));
        if !graph_cache.exists() {
            std::fs::create_dir(graph_cache.clone()).unwrap();
        }
        let png_file_path = graph_cache.join(format!("{}.png", self.pseudo_hash()));
        if !png_file_path.exists() {
            let dot = {
                let mut dot : String = "digraph MNet {\n".into();
                for p in self.places.keys() {
                    dot += &format!("{}[label=\"{}\" shape=ellipse];\n", p, p);
                }
                for e in self.edges.keys() {
                    dot += &format!("{}[label=\"{}\" shape=diamond];\n", e, e);
                }
                for connection_set in [&self.places_to_edges, &self.edges_to_places] {
                    for (source, sinks) in connection_set {
                        for sink in sinks {
                            dot += &format!("{} -> {}[label=\"\"];\n", source, sink);
                        }
                    }
                }
                dot += "}";
                dot
            };
            let mut dot_file = NamedTempFile::new().unwrap();
            dot_file.write_all(dot.as_bytes()).unwrap();
            dot_file.flush().unwrap();
            Command::new("dot").arg(dot_file.path()).arg("-Tpng").arg("-o").arg(&png_file_path).status().expect("dot failed");
        }
        png_file_path
    }
}

struct Worker {
    place_infos: HashMap<String, (HashSet<String>, PlaceMaker, HashMap<TypeId, String>)>,
    edges: HashMap<String, Edge>,
}
impl Worker {
    fn run(mut self) -> HashMap<String, Edge> {
        let mut places = self.place_infos.into_iter().map(|(name, (in_edges, make_place, out_edges))| (name, (in_edges, (make_place.make)(), out_edges)))
            .collect::<HashMap<String, (HashSet<String>, Box<dyn Place>, HashMap<TypeId, String>)>>();
        let mut continue_executing = true;
        while continue_executing {
            continue_executing = false;
            for (_place_name, (in_edges, place, out_edges_names)) in places.iter_mut() {
                for e in in_edges.iter() {
                    if self.edges[e].len() > 0 {
                        let input = self.edges.get_mut(e).unwrap().pop();
                        let mut out_edges = {
                            let mut out_edges = HashMap::new();
                            for (t, e_name) in out_edges_names.into_iter() {
                                out_edges.insert(*t, self.edges.remove(e_name).unwrap());
                            }
                            out_edges
                        };
                        place.run(input, &mut out_edges);
                        for (t, e_name) in out_edges_names.into_iter() {
                            self.edges.insert(e_name.clone(), out_edges.remove(&t).unwrap());
                        }
                        assert_eq!(out_edges.len(), 0);
                        continue_executing = true;
                    }
                }
            }
        }
        self.edges
    }
}

pub struct Runner {
    workers: Vec<Worker>,
}
impl Runner {
    pub fn from_maker(mut maker: Maker) -> Self {
        let mut places = HashMap::new();
        for (place_name, p) in maker.places.drain() {
            let in_edges = {
                let mut in_edges = HashSet::new();
                for (e, places) in &maker.edges_to_places {
                    if places.contains(&place_name) {
                        in_edges.insert(e.clone());
                        assert_eq!((p.in_type)(), maker.edges[e].type_id);
                    }
                }
                assert!(in_edges.len() > 0);
                in_edges
            };
            let out_edges = {
                let mut out_edges : HashMap<TypeId, String> = HashMap::new();
                for edge_name in &maker.places_to_edges[&place_name] {
                    //let edge = self.edges[edge_name].clone();
                    assert!(!out_edges.contains_key(&maker.edges[edge_name].type_id));
                    out_edges.insert(maker.edges[edge_name].type_id, edge_name.clone());
                }
                assert_eq!(out_edges.clone().into_keys().collect::<HashSet<TypeId>>(), (p.out_types)(),
                    "Place: {:?} has out edges: {:?}, but {:?} are not connected!", place_name,
                    (p.out_types_names)(),
                    (p.out_types_names)().difference(&maker.edges.iter().map(
                        |(_, e)| e.type_name.clone()
                    ).collect::<HashSet<String>>())
                );
                out_edges
            };
            places.insert(place_name.clone(), (in_edges, p, out_edges));
        }
        Self { workers: vec![Worker {place_infos: places, edges: maker.edges }] }
    }
    pub fn run(mut self: Self) -> HashMap<String, Edge> {
        let mut worker_threads = vec![];
        for worker in self.workers.drain(..) {
            worker_threads.push(thread::spawn(move || worker.run()));
        }
        let mut edges_end_state: HashMap<String, Edge> = HashMap::new();
        for worker in worker_threads.drain(..) {
            let mut out: HashMap<String, Edge> = worker.join().unwrap();
            for kv in out.drain() {
                let (edge_name, edge) = kv;
                if !edges_end_state.contains_key(&edge_name) {
                    edges_end_state.insert(edge_name, edge);
                }
                else {
                    //edges_end_state.get_mut(&edge_name).unwrap() = edge;
                }
            }
        }
        edges_end_state
    }
}