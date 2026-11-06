//! Retain native image identities only for the currently projected story rows.
use librairii_desktop::browsing::Story;
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

struct Cover {
    pixels: Weak<[u8]>,
    width: u32,
    height: u32,
    image: slint::Image,
}

#[derive(Default)]
pub struct CoverImages {
    visible: BTreeMap<String, Cover>,
}

impl CoverImages {
    pub fn clear(&mut self) {
        self.visible.clear();
    }

    pub fn project<'a>(&mut self, stories: impl Iterator<Item = &'a Story>) -> Vec<slint::Image> {
        let previous = std::mem::take(&mut self.visible);
        let mut visible: BTreeMap<String, Cover> = BTreeMap::new();
        let images = stories
            .map(|story| {
                let Some(thumbnail) = &story.thumbnail else {
                    return slint::Image::default();
                };
                let pixels = Arc::downgrade(&thumbnail.rgba);
                let image = visible
                    .get(&story.uuid)
                    .or_else(|| previous.get(&story.uuid))
                    .filter(|cover| {
                        cover.width == thumbnail.width
                            && cover.height == thumbnail.height
                            && cover.pixels.ptr_eq(&pixels)
                    })
                    .map(|cover| cover.image.clone())
                    .unwrap_or_else(|| {
                        slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
                            &thumbnail.rgba,
                            thumbnail.width,
                            thumbnail.height,
                        ))
                    });
                visible.insert(
                    story.uuid.clone(),
                    Cover {
                        pixels,
                        width: thumbnail.width,
                        height: thumbnail.height,
                        image: image.clone(),
                    },
                );
                image
            })
            .collect();
        // An old page cannot grow a second thumbnail cache. Weak identity also
        // avoids keeping decoded pixels alive beyond the worker's own LRU.
        self.visible = visible;
        images
    }
}
