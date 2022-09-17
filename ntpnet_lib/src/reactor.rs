use std::collections::{HashMap, HashSet, VecDeque};
use std::any::TypeId;
use bimap::BiMap;

use crate::transition::{Transition, Description};
use crate::{Token, TransitionMaker, net::Net};

#[derive(Debug)]
struct TransitionRuntime {
    t: Box<dyn Transition>,
    description: Description,
    in_edge_to_place: BiMap<String, String>,
    out_edge_to_place: BiMap<String, String>,
}

#[derive(Debug)]
struct State {
    places: HashMap<String, HashMap<TypeId, VecDeque<Token>>>,
    state: HashMap<(String, TypeId), usize>,
    state_exists: HashSet<(String, TypeId)>,
}
impl State {
    fn make(places: HashMap<String, HashMap<TypeId, VecDeque<Token>>>, transitions: &HashMap<String, TransitionRuntime>) -> Self {
        let state = {
            let mut state = HashMap::new();
            for (place_name, ty_v) in places.iter_mut() {
                for (ty, v) in ty_v.iter() {
                    state.insert((place_name.clone(), ty.clone()), v.len());
                }
                for ty in transitions.iter().fold(HashSet::new(), |acc, (_, t)| {
                    if let Some(edge_name) = t.in_edge_to_place.get_by_right(place_name) {
                        acc.union(&t.description.in_edges.iter().filter_map(|(e_name, ty)| {
                            if e_name == edge_name {
                                Some(ty.clone())
                            } else {
                                None
                            }
                        }).collect::<HashSet<_>>()).cloned().collect::<_>()
                    } else {
                        acc
                    }
                }).into_iter() {
                    ty_v.insert(ty, VecDeque::new());
                    state.insert((place_name.clone(), ty.clone()), 0);
                }
            }
            state
        };
        let state_exists = state.iter().filter_map(|(k, v)| if *v > 0 { Some(k.clone()) } else { None }).collect::<_>();

        Self {
            places: places,
            state: state,
            state_exists: state_exists,
        }
    }
    fn binary(&self) -> &HashSet<(String, TypeId)> {
        &self.state_exists
    }
    fn pop(&mut self, p_ty: &(String, TypeId)) -> Token {
        *self.state.get_mut(p_ty).unwrap() -= 1;
        if *self.state.get_mut(p_ty).unwrap() == 0 {
            self.state_exists.remove(p_ty);
        }
        self.places.get_mut(&p_ty.0).unwrap().get_mut(&p_ty.1).unwrap().pop_front().unwrap()
    }
}

#[derive(Debug)]
struct WorkCluster {
    transitions: HashMap<String, TransitionRuntime>,
    state: State,
}
impl WorkCluster {
    pub fn make(mut n: Net) -> Self {
        let transitions = n.transitions.into_iter()
            .map(|(name, t_maker)| {
                let t = t_maker();
                let mut d = t.description();
                let in_edge_to_place = n.pt_edges.iter()
                    .filter(|((_,t),_)| t == &name)
                    .map(|((p, _), e)| (e.clone(), p.clone())).collect::<BiMap<String, String>>();
                let out_edge_to_place = n.tp_edges.iter()
                    .filter(|((t,_),_)| t == &name)
                    .map(|((_, p), e)| (e.clone(), p.clone())).collect::<BiMap<String, String>>();
                for (_, case) in d.cases.iter_mut() {
                    for condition in case.conditions.iter_mut() {
                        *condition = condition.iter().map(|(edge, ty)| {
                            (
                                in_edge_to_place.get_by_left(edge).unwrap().clone(),
                                ty.clone(),
                            )
                        }).collect::<HashSet<_>>();
                    }
                    for product in case.products.iter_mut() {
                        *product = product.iter().map(|(edge, ty)| {
                            (
                                out_edge_to_place.get_by_left(edge).unwrap().clone(),
                                ty.clone(),
                            )
                        }).collect::<_>();
                    }
                }
                (name, TransitionRuntime {
                    t: t,
                    description: d,
                    in_edge_to_place: in_edge_to_place,
                    out_edge_to_place: out_edge_to_place,
                })
            })
            .collect::<HashMap<_,_>>();
        Self {
            state: State::make(n.places, &transitions),
            transitions: transitions,
        }
    }
    pub fn run(mut self) {
        for (name, t_run) in self.transitions.iter_mut() {
            for (f_name, case) in &t_run.description.cases {
                for (i, condition) in case.conditions.iter().enumerate() {
                    if (condition - self.state.binary()).len() == 0 {
                        let mut in_map = HashMap::new();
                        for p_ty in condition {
                            in_map.insert(
                                (t_run.in_edge_to_place.get_by_right(&p_ty.0).unwrap().clone(), p_ty.1.clone()),
                                self.state.pop(p_ty),
                            );
                        }
                        let mut out_map = HashMap::new();
                        t_run.t.call(&f_name, i, &mut in_map, &mut out_map);
                        for ((e_name, ty), t) in out_map.into_iter() {
                            let place = t_run.out_edge_to_place.get_by_left(&e_name).unwrap();
                            self.state.places.get_mut(place).unwrap().get_mut(&ty).unwrap().push_back(t);
                            *self.state.state.get_mut(&(place.clone(), ty)).unwrap() += 1;
                            if !self.state.state_exists.contains(&(place.clone(), ty)) {
                                self.state.state_exists.insert((place.clone(), ty));
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct Reactor {
    // TODO: mutiple work clusters
    work_cluster: WorkCluster,
}

impl Reactor {
    pub fn make(net: Net) -> Self {
        Self {
            work_cluster: WorkCluster::make(net),
        }
    }
    pub fn run(self) {
        println!("{:#?}", self.work_cluster);
        self.work_cluster.run();
        println!("Done!");
    }
}