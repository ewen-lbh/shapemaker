use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use rand::Rng;
use serde::Deserialize;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
pub enum Color {
    Black,
    White,
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Purple,
    Brown,
    Cyan,
    Pink,
    Gray,
}

#[wasm_bindgen]
pub fn random_color(except: Option<Color>) -> Color {
    let all = [
        Color::Black,
        Color::White,
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Yellow,
        Color::Orange,
        Color::Purple,
        Color::Brown,
        Color::Cyan,
        Color::Pink,
        Color::Gray,
    ];
    let candidates = all
        .iter()
        .filter(|c| match except {
            None => true,
            Some(color) => &&color != c,
        })
        .collect::<Vec<_>>();

    *candidates[rand::thread_rng().gen_range(0..candidates.len())]
}

pub fn all_colors() -> Vec<Color> {
    Color::iter().collect()
}

impl Default for Color {
    fn default() -> Self {
        Self::Black
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        match s {
            "black" => Color::Black,
            "white" => Color::White,
            "red" => Color::Red,
            "green" => Color::Green,
            "blue" => Color::Blue,
            "yellow" => Color::Yellow,
            "orange" => Color::Orange,
            "purple" => Color::Purple,
            "brown" => Color::Brown,
            "cyan" => Color::Cyan,
            "pink" => Color::Pink,
            "gray" => Color::Gray,
            _ => panic!("Invalid color: {}", s),
        }
    }
}

impl Color {
    pub fn render(self, mapping: &ColorMapping) -> String {
        match self {
            Color::Black => mapping.black.to_string(),
            Color::White => mapping.white.to_string(),
            Color::Red => mapping.red.to_string(),
            Color::Green => mapping.green.to_string(),
            Color::Blue => mapping.blue.to_string(),
            Color::Yellow => mapping.yellow.to_string(),
            Color::Orange => mapping.orange.to_string(),
            Color::Purple => mapping.purple.to_string(),
            Color::Brown => mapping.brown.to_string(),
            Color::Cyan => mapping.cyan.to_string(),
            Color::Pink => mapping.pink.to_string(),
            Color::Gray => mapping.gray.to_string(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Color::Black => "black",
            Color::White => "white",
            Color::Red => "red",
            Color::Green => "green",
            Color::Blue => "blue",
            Color::Yellow => "yellow",
            Color::Orange => "orange",
            Color::Purple => "purple",
            Color::Brown => "brown",
            Color::Cyan => "cyan",
            Color::Pink => "pink",
            Color::Gray => "gray",
        }
        .to_string()
    }
}

#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Deserialize, Clone)]
pub struct ColorMapping {
    pub black: String,
    pub white: String,
    pub red: String,
    pub green: String,
    pub blue: String,
    pub yellow: String,
    pub orange: String,
    pub purple: String,
    pub brown: String,
    pub cyan: String,
    pub pink: String,
    pub gray: String,
}

#[wasm_bindgen]
impl ColorMapping {
    pub fn default() -> Self {
        ColorMapping {
            black: "black".to_string(),
            white: "white".to_string(),
            red: "red".to_string(),
            green: "green".to_string(),
            blue: "blue".to_string(),
            yellow: "yellow".to_string(),
            orange: "orange".to_string(),
            purple: "purple".to_string(),
            brown: "brown".to_string(),
            pink: "pink".to_string(),
            gray: "gray".to_string(),
            cyan: "cyan".to_string(),
        }
    }

    pub fn from_json(content: &str) -> ColorMapping {
        let json: HashMap<String, String> = serde_json::from_str(content).unwrap();
        ColorMapping::from_hashmap(json)
    }

    pub fn from_css(content: &str) -> ColorMapping {
        let mut mapping = ColorMapping::default();
        for line in content.lines() {
            mapping.from_css_line(line);
        }
        mapping
    }
}

impl ColorMapping {
    pub fn from_cli_args(args: &Vec<String>) -> ColorMapping {
        let mut colormap: HashMap<String, String> = HashMap::new();
        for mapping in args {
            if !mapping.contains(':') {
                println!("Invalid color mapping: {}", mapping);
                std::process::exit(1);
            }
            let mut split = mapping.split(':');
            let color = split.next().unwrap();
            let hex = split.next().unwrap();
            colormap.insert(color.to_string(), hex.to_string());
        }
        ColorMapping::from_hashmap(colormap)
    }

    pub fn from_hashmap(hashmap: HashMap<String, String>) -> ColorMapping {
        ColorMapping {
            black: hashmap
                .get("black")
                .unwrap_or(&ColorMapping::default().black)
                .to_string(),
            white: hashmap
                .get("white")
                .unwrap_or(&ColorMapping::default().white)
                .to_string(),
            red: hashmap
                .get("red")
                .unwrap_or(&ColorMapping::default().red)
                .to_string(),
            green: hashmap
                .get("green")
                .unwrap_or(&ColorMapping::default().green)
                .to_string(),
            blue: hashmap
                .get("blue")
                .unwrap_or(&ColorMapping::default().blue)
                .to_string(),
            yellow: hashmap
                .get("yellow")
                .unwrap_or(&ColorMapping::default().yellow)
                .to_string(),
            orange: hashmap
                .get("orange")
                .unwrap_or(&ColorMapping::default().orange)
                .to_string(),
            purple: hashmap
                .get("purple")
                .unwrap_or(&ColorMapping::default().purple)
                .to_string(),
            brown: hashmap
                .get("brown")
                .unwrap_or(&ColorMapping::default().brown)
                .to_string(),
            cyan: hashmap
                .get("cyan")
                .unwrap_or(&ColorMapping::default().cyan)
                .to_string(),
            pink: hashmap
                .get("pink")
                .unwrap_or(&ColorMapping::default().pink)
                .to_string(),
            gray: hashmap
                .get("gray")
                .unwrap_or(&ColorMapping::default().gray)
                .to_string(),
        }
    }

    pub fn from_file(path: PathBuf) -> ColorMapping {
        match path.extension().map(|e| e.to_str().unwrap()) {
            Some("css") => ColorMapping::from_css_file(path),
            Some("json") => ColorMapping::from_json_file(path),
            ext => panic!(
                "Invalid colormap file format. Must be css or json, is {:?}.",
                ext
            ),
        }
    }

    pub fn from_json_file(path: PathBuf) -> ColorMapping {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let json: HashMap<String, String> = serde_json::from_reader(reader).unwrap();
        ColorMapping::from_hashmap(json)
    }

    pub fn from_css_file(path: PathBuf) -> ColorMapping {
        let mut mapping = ColorMapping::default();
        let file = File::open(path).unwrap();
        let lines = std::io::BufReader::new(file).lines();
        for line in lines {
            if let Ok(line) = line {
                mapping.from_css_line(&line);
            }
        }
        mapping
    }

    fn from_css_line(&mut self, line: &str) {
        if let Some((name, value)) = line.trim().split_once(':') {
            let value = value.trim().trim_end_matches(';').to_owned();
            match name.trim() {
                "black" => self.black = value,
                "white" => self.white = value,
                "red" => self.red = value,
                "green" => self.green = value,
                "blue" => self.blue = value,
                "yellow" => self.yellow = value,
                "orange" => self.orange = value,
                "purple" => self.purple = value,
                "brown" => self.brown = value,
                "cyan" => self.cyan = value,
                "pink" => self.pink = value,
                "gray" => self.gray = value,
                _ => (),
            }
        }
    }
}
