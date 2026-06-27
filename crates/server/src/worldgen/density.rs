//! Density-function interpreter.
//!
//! Vanilla shapes terrain with a graph of "density functions" (data, in
//! `density_function/*.json`): arithmetic, noise lookups, splines, gradients…
//! We parse that data into a tree and evaluate it with our own engine and noise.
//! Functions reference each other by id; references are resolved and inlined at
//! parse time (shared subtrees via `Rc`).
//!
//! NOTE: `old_blended_noise` (vanilla's base 3D terrain noise) is evaluated as 0
//! for now — its exact legacy seeding is the last piece before full parity; the
//! continents/erosion/depth/spline shaping is already real.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use serde_json::Value;

use super::noises::Noises;

/// A parsed, ready-to-evaluate density function.
pub enum Df {
    Const(f64),
    /// The block Y coordinate.
    YGradient {
        from_y: f64,
        to_y: f64,
        from_value: f64,
        to_value: f64,
    },
    Noise {
        key: String,
        xz_scale: f64,
        y_scale: f64,
    },
    ShiftedNoise {
        key: String,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Rc<Df>,
        shift_y: Rc<Df>,
        shift_z: Rc<Df>,
    },
    ShiftA(String),
    ShiftB(String),
    Add(Rc<Df>, Rc<Df>),
    Mul(Rc<Df>, Rc<Df>),
    Min(Rc<Df>, Rc<Df>),
    Max(Rc<Df>, Rc<Df>),
    Abs(Rc<Df>),
    Square(Rc<Df>),
    Cube(Rc<Df>),
    HalfNegative(Rc<Df>),
    QuarterNegative(Rc<Df>),
    Squeeze(Rc<Df>),
    Clamp {
        input: Rc<Df>,
        min: f64,
        max: f64,
    },
    RangeChoice {
        input: Rc<Df>,
        min: f64,
        max: f64,
        when_in: Rc<Df>,
        when_out: Rc<Df>,
    },
    Spline(Spline),
    /// Caching wrappers — transparent for value purposes here.
    Passthrough(Rc<Df>),
    /// Base 3D noise (vanilla `old_blended_noise`): 0 for now.
    BlendedZero,
}

/// A cubic spline over a coordinate density function.
pub struct Spline {
    coordinate: Rc<Df>,
    points: Vec<SplinePoint>,
}

struct SplinePoint {
    location: f64,
    value: SplineValue,
    derivative: f64,
}

enum SplineValue {
    Fixed(f64),
    Nested(Spline),
}

impl Df {
    pub fn eval(&self, x: f64, y: f64, z: f64, noises: &Noises) -> f64 {
        match self {
            Df::Const(c) => *c,
            Df::YGradient {
                from_y,
                to_y,
                from_value,
                to_value,
            } => clamped_map(y, *from_y, *to_y, *from_value, *to_value),
            Df::Noise { key, xz_scale, y_scale } => match noises.get(key) {
                Some(n) => n.get_value(x * xz_scale, y * y_scale, z * xz_scale),
                None => 0.0,
            },
            Df::ShiftedNoise {
                key,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => match noises.get(key) {
                Some(n) => {
                    let sx = x * xz_scale + shift_x.eval(x, y, z, noises);
                    let sy = y * y_scale + shift_y.eval(x, y, z, noises);
                    let sz = z * xz_scale + shift_z.eval(x, y, z, noises);
                    n.get_value(sx, sy, sz)
                }
                None => 0.0,
            },
            Df::ShiftA(key) => match noises.get(key) {
                Some(n) => n.get_value(x * 0.25, 0.0, z * 0.25) * 4.0,
                None => 0.0,
            },
            Df::ShiftB(key) => match noises.get(key) {
                Some(n) => n.get_value(z * 0.25, x * 0.25, 0.0) * 4.0,
                None => 0.0,
            },
            Df::Add(a, b) => a.eval(x, y, z, noises) + b.eval(x, y, z, noises),
            Df::Mul(a, b) => a.eval(x, y, z, noises) * b.eval(x, y, z, noises),
            Df::Min(a, b) => a.eval(x, y, z, noises).min(b.eval(x, y, z, noises)),
            Df::Max(a, b) => a.eval(x, y, z, noises).max(b.eval(x, y, z, noises)),
            Df::Abs(a) => a.eval(x, y, z, noises).abs(),
            Df::Square(a) => {
                let v = a.eval(x, y, z, noises);
                v * v
            }
            Df::Cube(a) => {
                let v = a.eval(x, y, z, noises);
                v * v * v
            }
            Df::HalfNegative(a) => {
                let v = a.eval(x, y, z, noises);
                if v > 0.0 { v } else { v * 0.5 }
            }
            Df::QuarterNegative(a) => {
                let v = a.eval(x, y, z, noises);
                if v > 0.0 { v } else { v * 0.25 }
            }
            Df::Squeeze(a) => {
                let v = a.eval(x, y, z, noises).clamp(-1.0, 1.0);
                v / 2.0 - v * v * v / 24.0
            }
            Df::Clamp { input, min, max } => input.eval(x, y, z, noises).clamp(*min, *max),
            Df::RangeChoice {
                input,
                min,
                max,
                when_in,
                when_out,
            } => {
                let v = input.eval(x, y, z, noises);
                if v >= *min && v < *max {
                    when_in.eval(x, y, z, noises)
                } else {
                    when_out.eval(x, y, z, noises)
                }
            }
            Df::Spline(s) => s.eval(x, y, z, noises),
            Df::Passthrough(a) => a.eval(x, y, z, noises),
            Df::BlendedZero => 0.0,
        }
    }
}

impl Spline {
    fn eval(&self, x: f64, y: f64, z: f64, noises: &Noises) -> f64 {
        let coord = self.coordinate.eval(x, y, z, noises);
        let pts = &self.points;
        if pts.is_empty() {
            return 0.0;
        }
        // Below the first / above the last point: linear extrapolation.
        if coord < pts[0].location {
            return pts[0].value.eval(x, y, z, noises)
                + pts[0].derivative * (coord - pts[0].location);
        }
        let last = pts.len() - 1;
        if coord >= pts[last].location {
            return pts[last].value.eval(x, y, z, noises)
                + pts[last].derivative * (coord - pts[last].location);
        }
        // Find the interval and do Hermite-style cubic interpolation.
        let mut i = 0;
        while i < last && coord >= pts[i + 1].location {
            i += 1;
        }
        let (p0, p1) = (&pts[i], &pts[i + 1]);
        let span = p1.location - p0.location;
        let t = (coord - p0.location) / span;
        let v0 = p0.value.eval(x, y, z, noises);
        let v1 = p1.value.eval(x, y, z, noises);
        let p = p0.derivative * span - (v1 - v0);
        let q = -p1.derivative * span + (v1 - v0);
        lerp(t, v0, v1) + t * (1.0 - t) * lerp(t, p, q)
    }
}

impl SplineValue {
    fn eval(&self, x: f64, y: f64, z: f64, noises: &Noises) -> f64 {
        match self {
            SplineValue::Fixed(v) => *v,
            SplineValue::Nested(s) => s.eval(x, y, z, noises),
        }
    }
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

/// Vanilla `Mth.clampedMap`: clamp `v` to `[from_in, to_in]` then map linearly.
fn clamped_map(v: f64, from_in: f64, to_in: f64, from_out: f64, to_out: f64) -> f64 {
    if v < from_in {
        from_out
    } else if v > to_in {
        to_out
    } else {
        from_out + (to_out - from_out) * (v - from_in) / (to_in - from_in)
    }
}

/// Loads and parses density functions on demand, caching shared subtrees.
pub struct Loader {
    dir: PathBuf,
    cache: HashMap<String, Rc<Df>>,
}

impl Loader {
    pub fn new(density_function_dir: PathBuf) -> Self {
        Self {
            dir: density_function_dir,
            cache: HashMap::new(),
        }
    }

    /// Parses the density function referenced by `id` (e.g. `"minecraft:overworld/depth"`).
    pub fn load(&mut self, id: &str) -> Rc<Df> {
        if let Some(df) = self.cache.get(id) {
            return Rc::clone(df);
        }
        // Insert a placeholder constant to break any accidental cycles.
        self.cache.insert(id.to_string(), Rc::new(Df::Const(0.0)));

        let path = self.dir.join(format!("{}.json", id.trim_start_matches("minecraft:")));
        let df = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(value) => self.parse(&value),
                Err(_) => Rc::new(Df::Const(0.0)),
            },
            Err(_) => Rc::new(Df::Const(0.0)),
        };
        self.cache.insert(id.to_string(), Rc::clone(&df));
        df
    }

    /// Parses a density-function JSON value (number, reference string, or object).
    pub fn parse(&mut self, v: &Value) -> Rc<Df> {
        match v {
            Value::Number(n) => Rc::new(Df::Const(n.as_f64().unwrap_or(0.0))),
            Value::String(id) => self.load(id),
            Value::Object(map) => self.parse_object(map),
            _ => Rc::new(Df::Const(0.0)),
        }
    }

    fn arg(&mut self, map: &serde_json::Map<String, Value>, key: &str) -> Rc<Df> {
        match map.get(key) {
            Some(v) => self.parse(v),
            None => Rc::new(Df::Const(0.0)),
        }
    }

    fn parse_object(&mut self, map: &serde_json::Map<String, Value>) -> Rc<Df> {
        let ty = map.get("type").and_then(Value::as_str).unwrap_or("");
        let num = |k: &str| map.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let noise_key = |k: &str| {
            map.get(k)
                .and_then(Value::as_str)
                .unwrap_or("minecraft:zero")
                .to_string()
        };
        let df = match ty.trim_start_matches("minecraft:") {
            "y_clamped_gradient" => Df::YGradient {
                from_y: num("from_y"),
                to_y: num("to_y"),
                from_value: num("from_value"),
                to_value: num("to_value"),
            },
            "noise" => Df::Noise {
                key: noise_key("noise"),
                xz_scale: num("xz_scale"),
                y_scale: num("y_scale"),
            },
            "shifted_noise" => Df::ShiftedNoise {
                key: noise_key("noise"),
                xz_scale: num("xz_scale"),
                y_scale: num("y_scale"),
                shift_x: self.arg(map, "shift_x"),
                shift_y: self.arg(map, "shift_y"),
                shift_z: self.arg(map, "shift_z"),
            },
            "shift_a" => Df::ShiftA(noise_key("argument")),
            "shift_b" => Df::ShiftB(noise_key("argument")),
            "add" => Df::Add(self.arg(map, "argument1"), self.arg(map, "argument2")),
            "mul" => Df::Mul(self.arg(map, "argument1"), self.arg(map, "argument2")),
            "min" => Df::Min(self.arg(map, "argument1"), self.arg(map, "argument2")),
            "max" => Df::Max(self.arg(map, "argument1"), self.arg(map, "argument2")),
            "abs" => Df::Abs(self.arg(map, "argument")),
            "square" => Df::Square(self.arg(map, "argument")),
            "cube" => Df::Cube(self.arg(map, "argument")),
            "half_negative" => Df::HalfNegative(self.arg(map, "argument")),
            "quarter_negative" => Df::QuarterNegative(self.arg(map, "argument")),
            "squeeze" => Df::Squeeze(self.arg(map, "argument")),
            "clamp" => Df::Clamp {
                input: self.arg(map, "input"),
                min: num("min"),
                max: num("max"),
            },
            "range_choice" => Df::RangeChoice {
                input: self.arg(map, "input"),
                min: num("min_inclusive"),
                max: num("max_exclusive"),
                when_in: self.arg(map, "when_in_range"),
                when_out: self.arg(map, "when_out_of_range"),
            },
            "flat_cache" | "cache_2d" | "cache_once" | "cache_all_in_cell" | "interpolated" => {
                Df::Passthrough(self.arg(map, "argument"))
            }
            "blend_alpha" => Df::Const(1.0),
            "blend_offset" => Df::Const(0.0),
            "blend_density" => Df::Passthrough(self.arg(map, "argument")),
            "spline" => Df::Spline(self.parse_spline(map.get("spline"))),
            // Base 3D noise and dimension-specific samplers: not yet exact.
            "old_blended_noise" | "end_islands" | "weird_scaled_sampler" | "interval_select" => {
                Df::BlendedZero
            }
            _ => Df::Const(0.0),
        };
        Rc::new(df)
    }

    fn parse_spline(&mut self, v: Option<&Value>) -> Spline {
        // A spline may itself be a bare constant.
        let Some(Value::Object(map)) = v else {
            let c = v.and_then(Value::as_f64).unwrap_or(0.0);
            return Spline {
                coordinate: Rc::new(Df::Const(0.0)),
                points: vec![SplinePoint {
                    location: 0.0,
                    value: SplineValue::Fixed(c),
                    derivative: 0.0,
                }],
            };
        };
        let coordinate = self.arg(map, "coordinate");
        let mut points = Vec::new();
        if let Some(Value::Array(arr)) = map.get("points") {
            for pt in arr {
                let Some(pt) = pt.as_object() else { continue };
                let location = pt.get("location").and_then(Value::as_f64).unwrap_or(0.0);
                let derivative = pt.get("derivative").and_then(Value::as_f64).unwrap_or(0.0);
                let value = match pt.get("value") {
                    Some(Value::Number(n)) => SplineValue::Fixed(n.as_f64().unwrap_or(0.0)),
                    other => SplineValue::Nested(self.parse_spline(other)),
                };
                points.push(SplinePoint {
                    location,
                    value,
                    derivative,
                });
            }
        }
        Spline { coordinate, points }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_map_endpoints() {
        assert_eq!(clamped_map(-10.0, 0.0, 10.0, 1.0, 2.0), 1.0);
        assert_eq!(clamped_map(20.0, 0.0, 10.0, 1.0, 2.0), 2.0);
        assert_eq!(clamped_map(5.0, 0.0, 10.0, 0.0, 10.0), 5.0);
    }

    #[test]
    fn parses_constants_and_arithmetic() {
        let mut loader = Loader::new(PathBuf::from("/nonexistent"));
        let noises = Noises::default();
        let v: Value = serde_json::json!({
            "type": "minecraft:add",
            "argument1": 2.0,
            "argument2": { "type": "minecraft:mul", "argument1": 3.0, "argument2": 4.0 }
        });
        let df = loader.parse(&v);
        assert_eq!(df.eval(0.0, 0.0, 0.0, &noises), 14.0);
    }
}
