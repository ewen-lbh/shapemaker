use crate::{ColorMapping, ColoredObject, Fill, Filter, ObjectSizes, Region};
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Clone, Default)]
// #[wasm_bindgen(getter_with_clone)]
pub struct Layer {
    pub object_sizes: ObjectSizes,
    pub objects: HashMap<String, ColoredObject>,
    pub name: String,
    pub _render_cache: Option<svg::node::element::Group>,
}

impl Layer {
    pub fn new(name: &str) -> Self {
        Layer {
            object_sizes: ObjectSizes::default(),
            objects: HashMap::new(),
            name: name.to_string(),
            _render_cache: None,
        }
    }

    pub fn object(&mut self, name: &str) -> &mut ColoredObject {
        self.objects.get_mut(name).unwrap()
    }

    // Flush the render cache.
    pub fn flush(&mut self) -> () {
        self._render_cache = None;
    }

    pub fn replace(&mut self, with: Layer) -> () {
        self.objects = with.objects.clone();
        self.flush();
    }

    pub fn remove_all_objects_in(&mut self, region: &Region) {
        self.objects
            .retain(|_, ColoredObject(o, ..)| !o.region().within(region))
    }

    pub fn paint_all_objects(&mut self, fill: Fill) {
        for (_id, ColoredObject(_, ref mut maybe_fill, _)) in &mut self.objects {
            *maybe_fill = Some(fill.clone());
        }
        self.flush();
    }

    pub fn filter_all_objects(&mut self, filter: Filter) {
        for (_id, ColoredObject(_, _, ref mut filters)) in &mut self.objects {
            filters.push(filter)
        }
        self.flush();
    }

    pub fn move_all_objects(&mut self, dx: i32, dy: i32) {
        self.objects
            .iter_mut()
            .for_each(|(_, ColoredObject(obj, _, _))| obj.translate(dx, dy));
        self.flush();
    }

    pub fn add_object<'a, N: Display>(&mut self, name: N, object: ColoredObject) {
        let name_str = format!("{}", name);

        if self.objects.contains_key(&name_str) {
            panic!("object {} already exists in layer {}", name_str, self.name);
        }

        self.objects.insert(name_str, object);
        self.flush();
    }

    pub fn filter_object(&mut self, name: &str, filter: Filter) -> Result<(), String> {
        self.objects
            .get_mut(name)
            .ok_or(format!("Object '{}' not found", name))?
            .2
            .push(filter);

        self.flush();
        Ok(())
    }

    pub fn remove_object(&mut self, name: &str) {
        self.objects.remove(name);
        self.flush();
    }

    pub fn replace_object(&mut self, name: &str, object: ColoredObject) {
        self.remove_object(name);
        self.add_object(name, object);
    }

    /// Render the layer to a SVG group element.
    pub fn render(
        &mut self,
        colormap: ColorMapping,
        cell_size: usize,
        object_sizes: ObjectSizes,
    ) -> svg::node::element::Group {
        if let Some(cached_svg) = &self._render_cache {
            return cached_svg.clone();
        }

        let mut layer_group = svg::node::element::Group::new()
            .set("class", "layer")
            .set("data-layer", self.name.clone());

        for (id, ColoredObject(object, maybe_fill, filters)) in &self.objects {
            layer_group = layer_group.add(object.render(
                cell_size,
                object_sizes,
                &colormap,
                &id,
                *maybe_fill,
                filters,
            ));
        }

        self._render_cache = Some(layer_group.clone());
        layer_group
    }
}
