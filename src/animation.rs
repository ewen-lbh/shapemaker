use std::fmt::Display;

use crate::{Canvas, Context, LaterHookCondition, RenderFunction};

/// Arguments: animation progress (from 0.0 to 1.0), canvas, current ms
pub type AnimationUpdateFunction = dyn Fn(f32, &mut Canvas, usize);

pub struct Animation {
    pub name: String,
    // pub keyframes: Vec<Keyframe<C>>,
    pub update: Box<AnimationUpdateFunction>,
}

// pub struct Keyframe<C: Default> {
//     pub at: f32, // from 0 to 1
//     pub action: Box<RenderFunction<C>>,
// }

impl Animation {
    /// Example
    /// ```
    /// Animation::new("example", &|t, canvas, _| {
    ///     canvas.root().object("dot").fill(Fill::Translucent(Color::Red, t))
    /// })
    /// ```
    pub fn new<N>(name: N, f: &'static AnimationUpdateFunction) -> Self
    where
        N: Display,
    {
        Self {
            name: format!("{}", name),
            update: Box::new(f),
        }
    }

    // /// Example:
    // /// ```
    // /// animation.at(50.0, Box::new(|canvas, _| canvas.root().set_background(Color::Black)));
    // /// ```
    // pub fn at(&mut self, percent: f32, action: Box<RenderFunction<C>>) {
    //     self.keyframes.push(Keyframe {
    //         at: percent / 100.0,
    //         action,
    //     });
    // }
}

impl From<(String, Box<AnimationUpdateFunction>)> for Animation {
    fn from((name, f): (String, Box<AnimationUpdateFunction>)) -> Self {
        Self { name, update: f }
    }
}
