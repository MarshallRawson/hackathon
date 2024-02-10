mod sin {
    use pyproc::PyProc;
    use ntpnet::plotmux::plotsink::PlotSink;
    use std::{thread, time};
    #[derive(ntpnet::TransitionInputTokensMacro, ntpnet::TransitionOutputTokensMacro)]
    struct Time {
        t: f64,
    }
    #[derive(ntpnet::Transition)]
    #[ntpnet_transition(sin: Input(Time) -> Output(Time))]
    pub struct Sin {
        p: PlotSink,
        py: PyProc,
    }
    impl Sin {
        pub fn maker(plotsink: PlotSink) -> ntpnet::TransitionMaker {
            Box::new(move || {
                let mut py = PyProc::new().unwrap();
                py.run("import math").unwrap();
                Box::new(Sin { p: plotsink, py: py })
            })
        }
        fn sin(&mut self, f: Input) -> Output {
            let t = match f {
                Input::Time(Time { t }) => t,
            };
            //let res = t.sin();
            let res: f64 = self.py.eval(&format!("math.sin({t})")).unwrap().parse().unwrap();
            self.p.plot_series_2d("", "sin(t)".into(), t, res);
            self.p.println2("t", &format!("{}", t));
            self.p.println2("sin(t)", &format!("{}", res));
            thread::sleep(time::Duration::from_millis(10));
            Output::Time(Time { t: t + 0.01 })
        }
    }
}

use ntpnet::{reactor, Net, Token};
use ntpnet::plotmux::plotmux::{ClientMode, PlotMux};

fn main() {
    let mut plotmux = PlotMux::make(ClientMode::Local());
    let n = Net::make()
        .set_start_tokens("time", vec![Token::new(0.)])
        .place_to_transition("time", "t", "sin")
        .add_transition("sin", sin::Sin::maker(plotmux.add_plot_sink("sin")))
        .transition_to_place("sin", "t", "time");
    let png = n.png();
    let r = reactor(n, &mut plotmux);
    let pm = plotmux.make_ready(Some(&png));
    r.run(&None);
    drop(pm);
}
