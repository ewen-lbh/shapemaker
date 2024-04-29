use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

use itertools::Itertools;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq)]
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

impl Default for Color {
    fn default() -> Self {
        Self::Black
    }
}

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
    pub fn from_json_file(path: &str) -> ColorMapping {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader).unwrap();
        ColorMapping {
            black: json["black"].as_str().unwrap().to_string(),
            white: json["white"].as_str().unwrap().to_string(),
            red: json["red"].as_str().unwrap().to_string(),
            green: json["green"].as_str().unwrap().to_string(),
            blue: json["blue"].as_str().unwrap().to_string(),
            yellow: json["yellow"].as_str().unwrap().to_string(),
            orange: json["orange"].as_str().unwrap().to_string(),
            purple: json["purple"].as_str().unwrap().to_string(),
            brown: json["brown"].as_str().unwrap().to_string(),
            cyan: json["cyan"].as_str().unwrap().to_string(),
            pink: json["pink"].as_str().unwrap().to_string(),
            gray: json["gray"].as_str().unwrap().to_string(),
        }
    }

    pub fn from_css_file(path: &str) -> ColorMapping {
        let file = File::open(path).unwrap();
        let lines = std::io::BufReader::new(file).lines();
        let mut mapping = ColorMapping::default();
        for line in lines {
            if let Ok(line) = line {
                mapping.from_css_line(&line);
            }
        }
        mapping
    }

    fn from_css_line(&mut self, line: &str) {
        if let Some((name, value)) = line.trim().split_once(":") {
            let value = value.trim().to_owned();
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

impl Color {
    pub fn to_string(self, mapping: &ColorMapping) -> String {
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
}
