use async_channel::Sender;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub static mut PLOTTER: Option<Sender<crate::FromMain>> = None;

const BUFFER_SIZE: usize = 50;
const BUFFER_TIMEOUT: Duration = Duration::from_millis(100);

// plot name, subplot name
// e.g. Names("encoder one", "target")
pub(crate) type Names = (String, String);

#[derive(Debug, Default)]
pub(crate) struct PlotManager(std::collections::HashMap<Names, SubPlot>);

impl PlotManager {
    pub(crate) fn add_point(&mut self, plot_name: String, subplt_name: String, point: Point) {
        let names = (plot_name, subplt_name);
        match self.0.get_mut(&names) {
            Some(plot) => {
                plot.add_point(point);
            }
            None => {
                self.0.insert(names.clone(), SubPlot::new(names, point));
            }
        }
    }
    pub(crate) fn buffers_to_send(&mut self) -> Vec<(String, String, Buffer)> {
        let mut out = Vec::new();
        for plot in self.0.values_mut() {
            if plot.point_buffer.len() >= BUFFER_SIZE
                || (plot.last_update.elapsed() >= BUFFER_TIMEOUT && !plot.point_buffer.is_empty())
            {
                out.push((
                    plot.names.0.clone(),
                    plot.names.1.clone(),
                    plot.point_buffer.take(),
                ));
                plot.last_update = Instant::now();
            }
        }
        out
    }
}

#[derive(Debug)]
struct SubPlot {
    start: Instant,
    names: Names,
    point_buffer: Buffer,
    last_update: Instant,
}

impl SubPlot {
    pub fn new(names: Names, point: Point) -> Self {
        Self {
            names,
            start: Instant::now(),
            point_buffer: point.into(),
            last_update: Instant::now(),
        }
    }
    pub fn add_point(&mut self, point: Point) {
        match (&mut self.point_buffer, point) {
            (Buffer::Scalar(buf), Point::Scalar(p)) => buf.push((p.0.duration_since(self.start), p.1)),
            (Buffer::Vec2(buf), Point::Vec2(p)) =>buf.push((p.0.duration_since(self.start), p.1)),
            (Buffer::Vec3(buf), Point::Vec3(p)) => buf.push((p.0.duration_since(self.start), p.1)),
            _ => log::warn!("subplot {:?} received point {point:?} of wrong type. Is there more then one subplot with the same name?", self.names),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Buffer {
    Scalar(Vec<(Duration, f64)>),
    Vec2(Vec<(Duration, [f64; 2])>),
    Vec3(Vec<(Duration, [f64; 3])>),
}

impl Buffer {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Scalar(b) => b.len(),
            Self::Vec2(b) => b.len(),
            Self::Vec3(b) => b.len(),
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Scalar(b) => b.is_empty(),
            Self::Vec2(b) => b.is_empty(),
            Self::Vec3(b) => b.is_empty(),
        }
    }
    pub(crate) fn take(&mut self) -> Buffer {
        let r = self.clone();
        match self {
            Self::Scalar(s) => s.clear(),
            Self::Vec2(s) => s.clear(),
            Self::Vec3(s) => s.clear(),
        }
        r
    }
}

impl From<Point> for Buffer {
    fn from(point: Point) -> Self {
        match point {
            Point::Scalar(s) => Self::Scalar(vec![(Duration::ZERO, s.1)]),
            Point::Vec2(s) => Self::Vec2(vec![(Duration::ZERO, s.1)]),
            Point::Vec3(s) => Self::Vec3(vec![(Duration::ZERO, s.1)]),
        }
    }
}

#[macro_export]
macro_rules! plot {
    ($plt_name:expr, $point:expr) => {
        plot!($plt_name, $plt_name, $point)
    };
    ($plt_name:expr, $subplt_name:expr, $point:expr) => {
        #[allow(unsafe_code)]
        unsafe {
            if let Some(sender) = &*std::ptr::addr_of!($crate::plot::PLOTTER) {
                #[allow(unused_imports)]
                use $crate::plot::{A, B, C};
                match sender.try_send(crate::packets::FromMain::Point(
                    $plt_name.into(), $subplt_name.into(),
                    $point.into_plot_point(),
                )) {
                    Err(e) if sender.len() <= 1 =>
                    log::error!(
                        "Failed to send plot data to listener thread with \"{e}\". This should never happen."
                    ),
                    _ => {},
                };
            }
        }

    };
}

#[derive(Debug, Clone, Copy)]
pub enum Point {
    Scalar((Instant, f64)),
    Vec2((Instant, [f64; 2])),
    Vec3((Instant, [f64; 3])),
}

// specialisation
pub trait A: Into<f64> {
    fn into_plot_point(self) -> Point {
        Point::Scalar((Instant::now(), self.into()))
    }
}
impl<T: Into<f64>> A for T {}
pub trait B: Into<[f64; 2]> {
    fn into_plot_point(self) -> Point {
        Point::Vec2((Instant::now(), self.into()))
    }
}
impl<T: Into<[f64; 2]>> B for T {}
pub trait C: Into<[f64; 3]> {
    fn into_plot_point(self) -> Point {
        Point::Vec3((Instant::now(), self.into()))
    }
}
impl<T: Into<[f64; 3]>> C for T {}

#[cfg(test)]
mod tests {
    #[test]
    fn different_types() {
        plot!("", 3i32);
        plot!("", [3.2f64, 1.2]);

        struct Test([i32; 3]);
        impl Into<[f64; 3]> for Test {
            fn into(self) -> [f64; 3] {
                [self.0[0] as f64, self.0[1] as f64, self.0[2] as f64]
            }
        }
        let plt_name = "a";
        plot!(plt_name, Test([3, 1, 2]));
        plot!("plot", "some_subplot", Test([3, 1, 2]));
    }
}
