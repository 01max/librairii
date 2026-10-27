//! Collection callbacks use the real controller and an owned SQLite library.
use super::{Fixture, Result, check};
use librairii_core::{
    collection_codec::{Query, url::Scope},
    query::QueryRequest,
};
use librairii_desktop::{
    presentation::{Command, Snapshot},
    workflows::collections::Feedback,
};
use librairii_ui::native::DesktopWindow;

fn choose(fixture: &Fixture, ui: &DesktopWindow, id: i64) -> Result {
    let index = fixture
        .view
        .borrow()
        .snapshot
        .groups
        .iter()
        .filter(|g| matches!(g.scope, Scope::Collection(_)))
        .position(|g| matches!(g.scope,Scope::Collection(current) if current.get()==id))
        .ok_or("Collection missing")?;
    ui.invoke_select_collection(index as i32 + 1);
    fixture.settle(ui)?;
    Ok(())
}
fn saved(snapshot: &Snapshot) -> Result<i64> {
    match snapshot.workflows.collections.feedback {
        Some(Feedback::Saved(id)) => Ok(id),
        _ => Err(format!(
            "Collection was not saved: {:?}",
            snapshot.workflows.collections.feedback
        )
        .into()),
    }
}
fn create(fixture: &Fixture, ui: &DesktopWindow, name: &str, search: &str) -> Result<i64> {
    fixture.search(ui, search)?;
    ui.invoke_show_panel(9);
    ui.invoke_select_collection(0);
    ui.set_collection_name(name.into());
    ui.invoke_save_collection();
    saved(&fixture.settle(ui)?)
}
fn actor(fixture: &Fixture) -> Result<rusqlite::Connection> {
    Ok(rusqlite::Connection::open(
        fixture._roots.library().join("native/db/librairii.sqlite3"),
    )?)
}
pub fn checks(ui: &DesktopWindow) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    let fixture = Fixture::open(ui)?;
    let alpha = create(&fixture, ui, "Alpha saved", "Alpha")?;
    ui.set_collection_name("Reverse".into());
    ui.invoke_save_collection();
    check(
        saved(&fixture.settle(ui)?)? == alpha,
        "Collection/Shelf namespaces collided",
    )?;
    ui.set_collection_name("Alpha copy".into());
    ui.invoke_duplicate_collection();
    let copy = saved(&fixture.settle(ui)?)?;
    ui.invoke_move_collection(-1);
    fixture.settle(ui)?;
    let beta = create(&fixture, ui, "Beta saved", "Beta")?;
    // A selected union is copied even while an unrelated query is displayed.
    for id in [alpha, beta, copy] {
        choose(&fixture, ui, id)?;
        ui.invoke_toggle_selected_collection_union();
        fixture.settle(ui)?;
    }
    let preview = fixture
        .view
        .borrow()
        .snapshot
        .workflows
        .collections
        .union
        .clone()
        .ok_or("union missing")?;
    check(
        preview.unique == 60 && preview.overlap == 30 && preview.sources.iter().all(|s| s.2 == 30),
        "union counts were not deduplicated",
    )?;
    fixture.search(ui, "unrelated")?;
    ui.invoke_select_collection(0);
    ui.set_collection_name("Combined".into());
    // Change a selected definition after the preview. Save must copy CURRENT data.
    let mut connection = actor(&fixture)?;
    let tx = connection.transaction()?;
    let mut criteria = Query::default();
    criteria.name = "Beta".into();
    let plan_payload = librairii_storage::collections::execute(
        &tx,
        librairii_storage::collections::Command::Create {
            name: "Replacement source".into(),
            criteria,
        },
    )?;
    let librairii_storage::collections::Response::Collection(replacement) = plan_payload else {
        return Err("source fixture missing".into());
    };
    tx.execute(
        "UPDATE collections SET query_payload=?1 WHERE id=?2",
        rusqlite::params![String::from_utf8(replacement.query_payload)?, alpha],
    )?;
    tx.commit()?;
    ui.invoke_save_collection();
    let combined = saved(&fixture.settle(ui)?)?;
    check(
        fixture
            .view
            .borrow()
            .snapshot
            .page
            .as_ref()
            .is_some_and(|p| p.total == 60),
        "primary save ignored selected union",
    )?;
    let payload: Vec<u8> = connection.query_row(
        "SELECT CAST(query_payload AS BLOB) FROM collections WHERE id=?",
        [combined],
        |r| r.get(0),
    )?;
    let copied: serde_json::Value = serde_json::from_slice(&payload)?;
    check(
        copied["anyOf"][0]["name"] == "beta",
        "union save copied the stale preview definition",
    )?;
    // Sort/page size survive Collection, duplicate, union and history navigation.
    let request = QueryRequest {
        sort: "imported_desc".into(),
        page_size: 17,
        ..Default::default()
    };
    fixture
        .worker
        .borrow()
        .offer(Command::Request(request.clone()))?;
    fixture.settle(ui)?;
    choose(&fixture, ui, beta)?;
    ui.invoke_open_collection();
    let opened = fixture.settle(ui)?;
    check(
        opened.navigation.as_ref().is_some_and(|n| {
            n.current().location.request.sort == request.sort
                && n.current().location.request.page_size == 17
                && n.current().location.request.page == 1
        }),
        "Collection open reset sort/page size",
    )?;
    ui.invoke_history_step(-1);
    fixture.settle(ui)?;
    ui.invoke_history_step(1);
    fixture.settle(ui)?;
    // Replacement is explicit, cancel restores the exact invoker, stale plans refuse.
    ui.invoke_show_panel(9);
    choose(&fixture, ui, beta)?;
    ui.invoke_replace_collection();
    fixture.settle(ui)?;
    check(
        ui.get_dialog() == 14 && ui.get_collection_cancel_focused(),
        "replacement did not focus Cancel",
    )?;
    ui.invoke_close_dialog();
    check(
        ui.get_dialog() == 13 && ui.get_collection_invoker_focused(),
        "replacement cancel lost invoker",
    )?;
    ui.invoke_replace_collection();
    fixture.settle(ui)?;
    connection.execute(
        "UPDATE collections SET name='Beta changed' WHERE id=?",
        [beta],
    )?;
    ui.invoke_confirm_collection_change();
    fixture.settle(ui)?;
    check(
        ui.get_collection_notice().contains("preview changed"),
        "stale replacement was applied",
    )?;
    ui.invoke_close_dialog();
    // Corrupt bytes survive reads; explicit replacement immediately clears attention.
    connection.execute("UPDATE collections SET query_payload='{\"name\":[]}',validity_state='needs_attention' WHERE id=?",[beta])?;
    fixture.worker.borrow().offer(Command::Refresh)?;
    fixture.settle(ui)?;
    choose(&fixture, ui, beta)?;
    check(
        !ui.get_collection_openable() && ui.get_collection_status().contains("corrupt"),
        "corrupt Collection looked healthy",
    )?;
    ui.invoke_open_collection();
    let refused = fixture.settle(ui)?;
    check(
        refused.phase == librairii_desktop::presentation::Phase::Library,
        "unavailable Collection fenced the library",
    )?;
    let raw: String = connection.query_row(
        "SELECT query_payload FROM collections WHERE id=?",
        [beta],
        |r| r.get(0),
    )?;
    check(
        raw == r#"{"name":[]}"#,
        "inspection rewrote invalid saved bytes",
    )?;
    fixture.search(ui, "Alpha")?;
    ui.invoke_replace_collection();
    fixture.settle(ui)?;
    ui.invoke_confirm_collection_change();
    fixture.settle(ui)?;
    check(
        ui.get_collection_openable() && ui.get_collection_status().contains("Live saved"),
        "repaired Collection kept stale attention",
    )?;
    // Deletion previews one affected Shelf at a time, and Keep-all retains UUID order.
    connection.execute(
        "INSERT INTO shelves(id,name,normalized_name,position) VALUES(2,'Second','second',1)",
        [],
    )?;
    for shelf in [1, 2] {
        connection.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(?1,?2,'Beta changed')",(shelf,beta))?;
    }
    ui.invoke_delete_collection();
    let planned = fixture.settle(ui)?;
    let preview = planned
        .workflows
        .collections
        .preview
        .as_ref()
        .ok_or("delete preview missing")?;
    check(
        preview.shelf_count == 2
            && preview.shelves.len() == 1
            && preview.page == 0
            && ui.get_collection_impact_next(),
        "deletion did not bound one Shelf per page",
    )?;
    ui.invoke_collection_impact_page(1);
    let next = fixture.settle(ui)?;
    check(
        next.workflows
            .collections
            .preview
            .as_ref()
            .is_some_and(|p| p.shelves.len() == 1 && p.page == 1)
            && !ui.get_collection_impact_next(),
        "deletion Shelf paging failed",
    )?;
    let before: Vec<String> = connection
        .prepare("SELECT story_uuid FROM shelf_stories WHERE shelf_id=1 ORDER BY position")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    ui.invoke_confirm_collection_change();
    fixture.settle(ui)?;
    let after: Vec<String> = connection
        .prepare("SELECT story_uuid FROM shelf_stories WHERE shelf_id=1 ORDER BY position")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    check(
        before == after
            && fixture
                .view
                .borrow()
                .snapshot
                .navigation
                .as_ref()
                .is_some_and(|n| n.current().location.scope == Scope::Custom),
        "deleted Collection changed approved order or active query scope",
    )?;
    let retained: Vec<u8> = connection.query_row(
        "SELECT CAST(query_payload AS BLOB) FROM collections WHERE id=?",
        [combined],
        |r| r.get(0),
    )?;
    check(retained == payload, "source mutation changed copied union")?;
    ui.invoke_close_dialog();
    fixture
        .worker
        .borrow()
        .offer(Command::Navigate(Scope::Library))?;
    let library = fixture.settle(ui)?;
    let page = library.page.as_ref().ok_or("Library page missing")?;
    check(
        !page.previews.is_empty()
            && page.previews.len() <= 10
            && page.previews.iter().all(|p| p.stories.len() <= 6),
        "Library group previews are absent or unbounded",
    )?;
    let index = page
        .all_stories()
        .enumerate()
        .find(|(_, s)| !page.stories.iter().any(|main| main.uuid == s.uuid))
        .map(|(i, _)| i)
        .ok_or("outside-page preview missing")?;
    fixture.select(ui, index as i32)?;
    ui.invoke_request_details();
    fixture.settle(ui)?;
    check(
        !ui.get_selected_story().uuid.is_empty() && ui.get_detail_flag_enabled(),
        "preview selection could not inspect/flag its real story",
    )?;
    drop(connection);
    fixture.finish()?;
    repair_checks(ui)
}

fn repair_checks(ui: &DesktopWindow) -> Result {
    let mut fixture = Fixture::open(ui)?;
    let mut connection = actor(&fixture)?;
    let mut query = Query::default();
    query.name = "Alpha".into();
    query
        .boolean_filters
        .push(librairii_core::collection_codec::BooleanFilter {
            definition_id: 101,
            state: "true".into(),
        });
    let tx = connection.transaction()?;
    let librairii_storage::collections::Response::Collection(collection) =
        librairii_storage::collections::execute(
            &tx,
            librairii_storage::collections::Command::Create {
                name: "Referenced".into(),
                criteria: query,
            },
        )?
    else {
        return Err("repair fixture missing".into());
    };
    tx.commit()?;
    connection.execute(
        "DELETE FROM story_tag_assignments WHERE definition_id=101",
        [],
    )?;
    connection.execute("DELETE FROM tag_definitions WHERE id=101", [])?;
    fixture.worker.borrow().offer(Command::Refresh)?;
    fixture.settle(ui)?;
    ui.invoke_show_panel(9);
    choose(&fixture, ui, collection.id)?;
    check(
        !ui.get_collection_openable() && ui.get_collection_status().contains("unavailable"),
        "missing reference was silently dropped",
    )?;
    ui.invoke_edit_saved_collection_query();
    let draft = fixture.settle(ui)?;
    check(
        ui.get_dialog() == 8
            && draft.page.is_none()
            && draft.navigation.as_ref().is_some_and(|n| {
                n.current().location.request.criteria.name == "alpha"
                    && n.current().location.request.criteria.boolean_filters.len() == 1
            }),
        "repair draft broadened missing criteria",
    )?;
    let raw: Vec<u8> = connection.query_row(
        "SELECT CAST(query_payload AS BLOB) FROM collections WHERE id=?",
        [collection.id],
        |r| r.get(0),
    )?;
    check(
        raw == collection.query_payload,
        "repair draft mutated saved bytes",
    )?;
    ui.invoke_filter_clear_tags();
    let repaired_draft = fixture.settle(ui)?;
    check(
        repaired_draft.page.as_ref().is_some_and(|p| p.total == 30) && ui.get_query() == "alpha",
        "explicit tag removal lost unrelated search",
    )?;
    ui.invoke_close_dialog();
    ui.invoke_show_panel(9);
    choose(&fixture, ui, collection.id)?;
    ui.invoke_replace_collection();
    fixture.settle(ui)?;
    ui.invoke_confirm_collection_change();
    fixture.settle(ui)?;
    check(
        ui.get_collection_openable() && ui.get_collection_status().contains("Live saved"),
        "missing-reference repair retained stale attention",
    )?;
    let empty = create(&fixture, ui, "Empty", "no matches")?;
    check(
        fixture
            .view
            .borrow()
            .snapshot
            .page
            .as_ref()
            .is_some_and(|p| p.total == 0),
        "empty saved Collection failed",
    )?;
    ui.set_collection_name("Referenced".into());
    ui.invoke_save_collection();
    fixture.settle(ui)?;
    check(
        ui.get_collection_notice().contains("already exists")
            && ui.get_collection_name() == "Referenced",
        "duplicate-name error lost the draft",
    )?;
    drop(connection);
    fixture.restart(ui)?;
    check(
        fixture.view.borrow().snapshot.groups.iter().any(|g| {
            matches!(g.scope,Scope::Collection(id) if id.get()==empty)
                && g.count == 0
                && g.available
        }),
        &format!(
            "empty Collection did not survive restart: {:?}",
            fixture.view.borrow().snapshot
        ),
    )?;
    ui.invoke_close_dialog();
    fixture.finish()
}

pub struct Surface(Fixture);
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectionSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        let fixture = Fixture::open(ui)?;
        actor(&fixture)?.execute_batch("WITH RECURSIVE ids(i) AS(VALUES(61) UNION ALL SELECT i+1 FROM ids WHERE i<100) INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) SELECT i,printf('10000000-0000-4000-8000-%012d',i),'Beta','beta' FROM ids; INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) SELECT id,'story.zip','zip',printf('%064d',id),1,'archives/'||id,'valid' FROM stories WHERE id>60;")?;
        for i in 0..8 {
            create(
                &fixture,
                ui,
                &format!("Collection {}", i + 1),
                if i % 2 == 0 { "Alpha" } else { "Beta" },
            )?;
        }
        ui.set_collection_name("Collection à garder · draft".into());
        Ok(Self(fixture))
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(snapshot) = self.0.worker.borrow().take_snapshot() {
            self.0
                .view
                .borrow_mut()
                .accept(ui, snapshot, self.0.worker.borrow().revision());
        }
        self.0.view.borrow_mut().resize(ui);
    }
    pub fn french(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        check(
            ui.get_collection_name() == "Collection à garder · draft",
            "language lost Collection draft",
        )
    }
    pub fn union(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_select_collection(8);
        self.0.settle(ui)?;
        ui.invoke_toggle_selected_collection_union();
        self.0.settle(ui)?;
        check(
            ui.get_collection_union_available(),
            "last Collection was unreachable",
        )?;
        ui.invoke_focus_collection_union();
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_collection_union_focused(),
            "union save was not reachable by native Tab",
        )?;
        Ok(())
    }
    pub fn impact(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_replace_collection();
        self.0.settle(ui)?;
        check(
            ui.get_collection_cancel_focused(),
            "shown replacement did not focus Cancel",
        )
    }
    pub fn overview(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        ui.invoke_close_dialog();
        ui.invoke_close_sidebar();
        self.0
            .worker
            .borrow()
            .offer(Command::Navigate(Scope::Library))?;
        self.0.settle(ui)?;
        self.0
            .worker
            .borrow()
            .offer(Command::Request(QueryRequest {
                page_size: 100,
                ..Default::default()
            }))?;
        self.0.settle(ui)?;
        Ok(())
    }
    pub fn bounded(&self, ui: &DesktopWindow) -> Result {
        check(
            ui.get_story_count() == 100 && ui.get_observed_browser_height() < 400.,
            "overview expanded the main ListView viewport",
        )?;
        ui.invoke_focus_browser();
        super::super::key(ui, slint::platform::Key::PageDown.into())?;
        self.0.settle(ui)?;
        check(
            ui.get_selected_index() > 0 && ui.get_selected_index() < 20,
            "overview PageDown jumped the complete page",
        )?;
        self.0
            .worker
            .borrow()
            .offer(Command::Request(QueryRequest {
                page_size: 24,
                ..Default::default()
            }))?;
        self.0.settle(ui)?;
        Ok(())
    }
    pub fn focus_preview(&self, ui: &DesktopWindow) -> Result {
        let index = {
            let view = self.0.view.borrow();
            let page = view.snapshot.page.as_ref().ok_or("page missing")?;
            page.all_stories()
                .enumerate()
                .find(|(_, s)| !page.stories.iter().any(|main| main.uuid == s.uuid))
                .map(|(i, _)| i)
                .ok_or("preview outside page missing")?
        };
        let expected = -1001 - index as i32 * 3;
        ui.invoke_focus_browser();
        for _ in 0..250 {
            super::super::key(ui, slint::platform::Key::Tab.into())?;
            if ui.get_focused_key() == expected {
                return Ok(());
            }
        }
        Err(format!(
            "preview was unreachable by native Tab: expected {expected}, actual {}",
            ui.get_focused_key()
        )
        .into())
    }
    pub fn preview(&self, ui: &DesktopWindow) -> Result {
        let index = usize::try_from((-1001 - ui.get_focused_key()) / 3)?;
        super::super::key(ui, slint::platform::Key::Space.into())?;
        self.0.settle(ui)?;
        check(
            ui.get_focused_key() == -1001 - index as i32 * 3
                && !ui.get_selected_story().uuid.is_empty(),
            "preview Space lost inspection/focus",
        )
    }
    pub fn bulk(&self, ui: &DesktopWindow) -> Result {
        let selected = self.0.view.borrow().selected.clone();
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        super::super::key(ui, slint::platform::Key::Space.into())?;
        check(
            self.0.view.borrow().tags.selection.len() == 1
                && self.0.view.borrow().tags.selection.first() == selected.as_ref()
                && self.0.view.borrow().selected == selected,
            "preview checkbox Space changed inspection or failed bulk selection",
        )
    }
}
