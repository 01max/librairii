// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Local Librairii correction: bounded macOS metadata indexing, preserving input order.
use crate::{
    family::{FamilyId, FamilyInfo},
    family_name::FamilyName,
    font::FontInfo,
    scan::{ScannedCollection, all_names, scan_paths},
    source::SourcePathMap,
};
use alloc::{string::String, vec, vec::Vec};
use hashbrown::HashMap;
use read_fonts::types::NameId;
use smallvec::SmallVec;
#[cfg(feature = "std")]
struct IndexedFont {
    names: Vec<String>,
    postscript: String,
    font: FontInfo,
}

#[cfg(feature = "std")]
fn index_records(
    paths: &[std::path::PathBuf],
    data: &std::sync::Mutex<SourcePathMap>,
) -> Vec<IndexedFont> {
    let mut records = Vec::new();
    let mut pool = vec![];
    let mut names = vec![];
    scan_paths(paths, 0, |scanned| {
        let Some(path) = scanned.path else { return };
        pool.append(&mut names);
        if !all_names(
            &scanned.name_table,
            NameId::TYPOGRAPHIC_FAMILY_NAME,
            &mut pool,
            &mut names,
        ) && !all_names(
            &scanned.name_table,
            NameId::FAMILY_NAME,
            &mut pool,
            &mut names,
        ) {
            return;
        }
        let Some(postscript) = scanned.english_or_first_name(NameId::POSTSCRIPT_NAME) else {
            return;
        };
        let postscript: String = postscript.chars().collect();
        let source = data.lock().unwrap().get_or_insert(path);
        let Some(font) = FontInfo::from_font_ref(&scanned.font, source, scanned.index) else {
            return;
        };
        if names.is_empty() {
            return;
        }
        records.push(IndexedFont {
            names: names.clone(),
            postscript,
            font,
        });
    });
    records
}

#[cfg(feature = "std")]
pub(crate) fn scan(paths: &[std::path::PathBuf]) -> ScannedCollection {
    let count = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(2);
    if count == 1 || paths.len() < 8 {
        return ScannedCollection::from_paths(paths, 0);
    }
    let data = std::sync::Mutex::new(SourcePathMap::default());
    let records = std::thread::scope(|scope| {
        let mut tasks = Vec::new();
        for chunk in paths.chunks(paths.len().div_ceil(count)) {
            let shared = &data;
            let task = std::thread::Builder::new()
                .name("font-index".into())
                .spawn_scoped(scope, move || index_records(chunk, shared));
            tasks.push(match task {
                Ok(handle) => Ok(handle),
                Err(_) => Err(index_records(chunk, shared)),
            });
        }
        tasks
            .into_iter()
            .flat_map(|task| match task {
                Ok(handle) => handle
                    .join()
                    .unwrap_or_else(|error| std::panic::resume_unwind(error)),
                Err(records) => records,
            })
            .collect::<Vec<_>>()
    });
    let mut collection = ScannedCollection {
        data_paths: data.into_inner().unwrap(),
        ..Default::default()
    };
    let mut families: HashMap<FamilyId, (FamilyName, SmallVec<[FontInfo; 4]>)> = HashMap::default();
    for record in records {
        let name = collection.family_names.get_or_insert(&record.names[0]);
        for alias in &record.names[1..] {
            collection.family_names.add_alias(name.id(), alias);
        }
        collection
            .postscript_names
            .insert(record.postscript, name.id());
        families
            .entry(name.id())
            .or_insert_with(|| (name.clone(), SmallVec::default()))
            .1
            .push(record.font);
    }
    collection.families.extend(
        families
            .into_iter()
            .map(|(id, (name, fonts))| (id, FamilyInfo::new(name, fonts))),
    );
    collection
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceKind;
    use std::{collections::BTreeMap, path::PathBuf, time::Instant};

    fn snapshot(collection: &ScannedCollection) -> BTreeMap<String, String> {
        let mut rows = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for alias in collection.family_names.iter() {
            let canonical = collection
                .family_names
                .get_by_id(alias.id())
                .unwrap()
                .name();
            let family = collection.families.get(&alias.id()).unwrap();
            let mut faces = Vec::new();
            for font in family.fonts() {
                if let SourceKind::Path(path) = font.source().kind() {
                    let identity = font.source().id().to_u64();
                    if let Some(prior) = sources.insert(path.to_path_buf(), identity) {
                        assert_eq!(prior, identity, "one source identity per file");
                    }
                }
                faces.push(alloc::format!(
                    "{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}",
                    font.source().kind(),
                    font.index(),
                    font.width(),
                    font.style(),
                    font.weight(),
                    font.axes(),
                    font.charmap_index()
                ));
            }
            rows.insert(
                alloc::format!("alias:{}", alias.name()),
                alloc::format!("{}|{:?}", canonical, faces),
            );
        }
        for (postscript, id) in &collection.postscript_names {
            rows.insert(
                alloc::format!("postscript:{}", postscript),
                collection
                    .family_names
                    .get_by_id(*id)
                    .unwrap()
                    .name()
                    .into(),
            );
        }
        let unique: std::collections::BTreeSet<_> = sources.values().collect();
        assert_eq!(
            unique.len(),
            sources.len(),
            "distinct files have distinct identities"
        );
        rows
    }

    #[test]
    fn bounded_index_preserves_complete_system_font_metadata_and_order() {
        let mut paths: Vec<PathBuf> = crate::backend::system_font_paths()
            .unwrap()
            .into_iter()
            .collect();
        paths.sort();
        assert!(paths.len() >= 8, "actual macOS system fonts required");
        for reverse in [false, true] {
            if reverse {
                paths.reverse();
            }
            let started = Instant::now();
            let original = ScannedCollection::from_paths(&paths, 0);
            let original_ms = started.elapsed().as_secs_f64() * 1000.;
            let started = Instant::now();
            let candidate = scan(&paths);
            let candidate_ms = started.elapsed().as_secs_f64() * 1000.;
            assert!(
                snapshot(&original) == snapshot(&candidate),
                "all aliases, PostScript names and complete face metadata must match"
            );
            std::eprintln!(
                "font_index_equivalence; files={}; reverse={reverse}; serial_ms={original_ms:.6}; bounded_parallel_ms={candidate_ms:.6}; complete_metadata_equal=true",
                paths.len()
            );
        }
    }
}
