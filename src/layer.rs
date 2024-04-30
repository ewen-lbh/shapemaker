use crate::{ColorMapping, Fill, Object, ObjectSizes};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub object_sizes: ObjectSizes,
    pub objects: HashMap<String, (Object, Option<Fill>)>,
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

    pub fn object(&mut self, name: &str) -> &mut (Object, Option<Fill>) {
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

    pub fn paint_all_objects(&mut self, fill: Fill) {
        for (_id, (_, maybe_fill)) in &mut self.objects {
            *maybe_fill = Some(fill.clone());
        }
        self.flush();
    }

    pub fn move_all_objects(&mut self, dx: i32, dy: i32) {
        self.objects
            .iter_mut()
            .for_each(|(_, (obj, _))| obj.translate(dx, dy));
        self.flush();
    }

    pub fn add_object(&mut self, name: &str, object: Object, fill: Option<Fill>) {
        self.objects.insert(name.to_string(), (object, fill));
        self.flush();
    }

    pub fn remove_object(&mut self, name: &str) {
        self.objects.remove(name);
        self.flush();
    }

    pub fn replace_object(&mut self, name: &str, object: Object, fill: Option<Fill>) {
        self.remove_object(name);
        self.add_object(name, object, fill);
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

        for (id, (object, maybe_fill)) in &self.objects {
            layer_group = layer_group.add(object.render(
                cell_size,
                object_sizes,
                &colormap,
                &id,
                *maybe_fill,
            ));
        }

        self._render_cache = Some(layer_group.clone());
        layer_group
    }
}
