use crate::packet::FromMediator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub static mut PLOTTER: Option<mpsc::Sender<FromMediator>> = None;

const BUFFER_SIZE: usize = 50;
const BUFFER_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Default)]
pub(crate) struct PlotManager(HashMap<String, Plot>);

impl PlotManager {
    pub(crate) fn add_point(&mut self, (name, point): (String, Point)) {
        self.0
            .entry(name.clone())
            .and_modify(|plot| plot.add_point(point))
            .or_insert(Plot::new(name, point));
    }
    pub(crate) fn buffers_to_send(&mut self) -> Vec<(String, Buffer)> {
        let mut out = Vec::new();
        for plot in self.0.values_mut() {
            if plot.point_buffer.len() >= BUFFER_SIZE
                || (plot.last_update.elapsed() >= BUFFER_TIMEOUT && !plot.point_buffer.is_empty())
            {
                out.push((plot.name.clone(), plot.point_buffer.take()));
                plot.last_update = Instant::now();
            }
        }
        out
    }
}

#[derive(Debug)]
struct Plot {
    start: Instant,
    name: String,
    point_buffer: Buffer,
    last_update: Instant,
}

impl Plot {
    pub fn new(name: String, point: Point) -> Self {
        Self {
            name,
            start: Instant::now(),
            point_buffer: point.into(),
            last_update: Instant::now(),
        }
    }
    pub fn add_point(&mut self, point: Point) {
        let dur = self.start.elapsed();
        match (&mut self.point_buffer, point) {
            (Buffer::Scalar(buf), Point::Scalar(p)) => buf.push((dur, p)),
            (Buffer::Vec2(buf), Point::Vec2(p)) => buf.push((dur, p)),
            (Buffer::Vec3(buf), Point::Vec3(p)) => buf.push((dur, p)),
            _ => log::warn!("plot {} received point {point:?} of wrong type. Is there more then one plot with the same name?", self.name),
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
            Point::Scalar(s) => Self::Scalar(vec![(Duration::ZERO, s)]),
            Point::Vec2(s) => Self::Vec2(vec![(Duration::ZERO, s)]),
            Point::Vec3(s) => Self::Vec3(vec![(Duration::ZERO, s)]),
        }
    }
}

#[macro_export]
macro_rules! plot {
    ($plt_name:expr, $point:expr) => {
        #[allow(unsafe_code)]
        unsafe {
            if let Some(sender) = &$crate::plot::PLOTTER {
                #[allow(unused_imports)]
                use $crate::plot::{A, B, C};
                if let Err(e) = sender.send($crate::packet::FromMediator::Point((
                    format!($plt_name),
                    $point.into_plot_point(),
                ))) {
                    log::error!(
                        "Failed to send plot data to listener thread with \"{e}\". This should never happen."
                    );
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy)]
pub enum Point {
    Scalar(f64),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
}

// specialisation
pub trait A: Into<f64> {
    fn into_plot_point(self) -> Point {
        Point::Scalar(self.into())
    }
}
impl<T: Into<f64>> A for T {}
pub trait B: Into<[f64; 2]> {
    fn into_plot_point(self) -> Point {
        Point::Vec2(self.into())
    }
}
impl<T: Into<[f64; 2]>> B for T {}
pub trait C: Into<[f64; 3]> {
    fn into_plot_point(self) -> Point {
        Point::Vec3(self.into())
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
        plot!("", Test([3, 1, 2]));
    }
}
