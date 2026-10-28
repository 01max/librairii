// Copyright 2022 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

// Derived from Chromium's accessibility abstraction.
// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE.chromium file.

use crate::{
    AdapterCallback, CacheEvent, Event, ObjectEvent, WindowEvent,
    context::{ActionHandlerNoMut, ActionHandlerWrapper, AppContext, Context},
    filters::filter,
    node::{NodeIdOrRoot, NodeWrapper, PlatformNode, PlatformRoot},
    util::WindowBounds,
};
use accesskit::{ActionHandler, Role, TreeUpdate};
use accesskit_consumer::{FilterResult, Node, NodeId, Tree, TreeChangeHandler, TreeState};
use atspi_common::{InterfaceSet, Politeness, State};
use std::fmt::{Debug, Formatter};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

struct AdapterChangeHandler<'a> {
    adapter: &'a Adapter,
    added_nodes: HashSet<NodeId>,
    removed_nodes: HashSet<NodeId>,
    checked_text_change: HashSet<NodeId>,
    selection_changed: HashSet<NodeId>,
    notified_nodes: HashSet<NodeId>,
    notified_children: HashSet<NodeId>,
}

impl<'a> AdapterChangeHandler<'a> {
    fn new(adapter: &'a Adapter) -> Self {
        Self {
            adapter,
            added_nodes: HashSet::new(),
            removed_nodes: HashSet::new(),
            checked_text_change: HashSet::new(),
            selection_changed: HashSet::new(),
            notified_nodes: HashSet::new(),
            notified_children: HashSet::new(),
        }
    }

    fn add_node(&mut self, node: &Node) {
        let id = node.id();
        if self.added_nodes.contains(&id) {
            return;
        }
        self.added_nodes.insert(id);

        let role = node.role();
        let is_root = node.is_root();
        let wrapper = NodeWrapper(node);
        let interfaces = wrapper.interfaces();
        let was_registered = self.adapter.is_registered(id);
        self.adapter.sync_interfaces(node.id(), interfaces);
        self.adapter.emit_cache_added(node.id());
        if was_registered {
            self.adapter
                .emit_object_event(id, ObjectEvent::StateChanged(State::Visible, true));
            self.adapter
                .emit_object_event(id, ObjectEvent::StateChanged(State::Showing, true));
        }
        if !was_registered && is_root && role == Role::Window {
            let adapter_index = self
                .adapter
                .context
                .read_app_context()
                .adapter_index(self.adapter.id)
                .unwrap();
            self.adapter.window_created(adapter_index, node.id());
        }

        let live = wrapper.live();
        if live != Politeness::None {
            if let Some(name) = wrapper.name() {
                self.adapter
                    .emit_object_event(node.id(), ObjectEvent::Announcement(name, live));
            }
        }
        if let Some(true) = node.is_selected() {
            self.enqueue_selection_changed_if_needed(node);
        }
    }

    fn add_subtree(&mut self, node: &Node) {
        self.add_node(node);
        for child in node.filtered_children(&filter) {
            self.add_subtree(&child);
        }
    }

    fn remove_node(&mut self, node: &Node) {
        let id = node.id();
        if self.removed_nodes.contains(&id) || !self.adapter.is_registered(id) {
            return;
        }
        self.removed_nodes.insert(id);

        let role = node.role();
        let is_root = node.is_root();
        if is_root && role == Role::Window {
            self.adapter.window_destroyed(node.id());
        }
        self.adapter
            .emit_object_event(node.id(), ObjectEvent::StateChanged(State::Defunct, true));
        self.adapter.emit_cache_removed(node.id());
        self.adapter.remove_interfaces(node.id());
        if let Some(true) = node.is_selected() {
            self.enqueue_selection_changed_if_needed(node);
        }
    }

    fn hide_subtree(&mut self, old_node: &Node, new_tree: &TreeState) {
        for child in old_node.filtered_children(&filter) {
            self.hide_subtree(&child, new_tree);
        }
        if let Some(new_node) = new_tree.node_by_id(old_node.id()) {
            self.notify_node(old_node, &new_node);
        }
    }

    fn notify_children(&mut self, old_node: &Node, new_node: &Node) {
        if self.adapter.is_registered(new_node.id()) && self.notified_children.insert(new_node.id())
        {
            NodeWrapper(new_node).notify_children_changes(self.adapter, &NodeWrapper(old_node));
        }
    }

    fn notify_parents(&mut self, old_node: &Node, new_node: &Node) {
        // A child's filter can change without its parent being in TreeUpdate.
        // Reconcile both exposed parents once against the complete old/new trees.
        let parents = [
            old_node.filtered_parent(&filter),
            new_node.filtered_parent(&filter),
        ];
        for parent in parents.into_iter().flatten() {
            if let (Some(old_parent), Some(new_parent)) = (
                old_node.tree_state.node_by_id(parent.id()),
                new_node.tree_state.node_by_id(parent.id()),
            ) {
                self.notify_children(&old_parent, &new_parent);
            }
        }
    }

    fn notify_node(&mut self, old_node: &Node, new_node: &Node) {
        if !self.adapter.is_registered(new_node.id()) || !self.notified_nodes.insert(new_node.id())
        {
            return;
        }
        let old = NodeWrapper(old_node);
        let new = NodeWrapper(new_node);
        self.adapter
            .sync_interfaces(new_node.id(), new.interfaces());
        if filter(new_node) == FilterResult::Include {
            let bounds = *self.adapter.context.read_root_window_bounds();
            new.notify_changes(&bounds, self.adapter, &old);
        } else {
            // Temporary clipping changes exposure, never logical lifetime.
            // Keep clients' existing references and avoid hidden live-region speech.
            new.notify_state_changes(self.adapter, &old);
        }
        self.notify_children(old_node, new_node);
    }

    fn emit_text_change_if_needed_parent(&mut self, old_node: &Node, new_node: &Node) {
        if !new_node.supports_text_ranges() || !old_node.supports_text_ranges() {
            return;
        }
        let id = new_node.id();
        if self.checked_text_change.contains(&id) {
            return;
        }
        self.checked_text_change.insert(id);
        let old_text = old_node.document_range().text();
        let new_text = new_node.document_range().text();

        let mut old_chars = old_text.chars();
        let mut new_chars = new_text.chars();
        let mut prefix_usv_count = 0;
        let mut prefix_byte_count = 0;
        loop {
            match (old_chars.next(), new_chars.next()) {
                (Some(old_char), Some(new_char)) if old_char == new_char => {
                    prefix_usv_count += 1;
                    prefix_byte_count += new_char.len_utf8();
                }
                (None, None) => return,
                _ => break,
            }
        }

        let suffix_byte_count = old_text[prefix_byte_count..]
            .chars()
            .rev()
            .zip(new_text[prefix_byte_count..].chars().rev())
            .take_while(|(old_char, new_char)| old_char == new_char)
            .fold(0, |count, (c, _)| count + c.len_utf8());

        let old_content = &old_text[prefix_byte_count..old_text.len() - suffix_byte_count];
        if let Ok(length) = old_content.chars().count().try_into() {
            if length > 0 {
                self.adapter.emit_object_event(
                    id,
                    ObjectEvent::TextRemoved {
                        start_index: prefix_usv_count,
                        length,
                        content: old_content.to_string(),
                    },
                );
            }
        }

        let new_content = &new_text[prefix_byte_count..new_text.len() - suffix_byte_count];
        if let Ok(length) = new_content.chars().count().try_into() {
            if length > 0 {
                self.adapter.emit_object_event(
                    id,
                    ObjectEvent::TextInserted {
                        start_index: prefix_usv_count,
                        length,
                        content: new_content.to_string(),
                    },
                );
            }
        }
    }

    fn emit_text_change_if_needed(&mut self, old_node: &Node, new_node: &Node) {
        if let Role::TextRun | Role::GenericContainer = new_node.role() {
            if let (Some(old_parent), Some(new_parent)) = (
                old_node.filtered_parent(&filter),
                new_node.filtered_parent(&filter),
            ) {
                self.emit_text_change_if_needed_parent(&old_parent, &new_parent);
            }
        } else {
            self.emit_text_change_if_needed_parent(old_node, new_node);
        }
    }

    fn emit_text_selection_change(&self, old_node: Option<&Node>, new_node: &Node) {
        if !new_node.supports_text_ranges() {
            return;
        }
        let Some(old_node) = old_node else {
            if let Some(selection) = new_node.text_selection() {
                if !selection.is_degenerate() {
                    self.adapter
                        .emit_object_event(new_node.id(), ObjectEvent::TextSelectionChanged);
                }
            }
            if let Some(selection_focus) = new_node.text_selection_focus() {
                if let Ok(offset) = selection_focus.to_global_usv_index().try_into() {
                    self.adapter
                        .emit_object_event(new_node.id(), ObjectEvent::CaretMoved(offset));
                }
            }
            return;
        };
        if !old_node.is_focused() || new_node.raw_text_selection() == old_node.raw_text_selection()
        {
            return;
        }

        if let Some(selection) = new_node.text_selection() {
            if !selection.is_degenerate()
                || old_node
                    .text_selection()
                    .map(|selection| !selection.is_degenerate())
                    .unwrap_or(false)
            {
                self.adapter
                    .emit_object_event(new_node.id(), ObjectEvent::TextSelectionChanged);
            }
        }

        let old_caret_position = old_node
            .raw_text_selection()
            .map(|selection| selection.focus);
        let new_caret_position = new_node
            .raw_text_selection()
            .map(|selection| selection.focus);
        if old_caret_position != new_caret_position {
            if let Some(selection_focus) = new_node.text_selection_focus() {
                if let Ok(offset) = selection_focus.to_global_usv_index().try_into() {
                    self.adapter
                        .emit_object_event(new_node.id(), ObjectEvent::CaretMoved(offset));
                }
            }
        }
    }

    fn enqueue_selection_changed_if_needed_parent(&mut self, node: Node) {
        if !node.is_container_with_selectable_children() {
            return;
        }
        let id = node.id();
        if self.selection_changed.contains(&id) {
            return;
        }
        self.selection_changed.insert(id);
    }

    fn enqueue_selection_changed_if_needed(&mut self, node: &Node) {
        if !node.is_item_like() {
            return;
        }
        if let Some(node) = node.selection_container(&filter) {
            self.enqueue_selection_changed_if_needed_parent(node);
        }
    }

    fn emit_selection_changed(&mut self) {
        for id in self.selection_changed.iter() {
            if self.removed_nodes.contains(id) {
                continue;
            }
            self.adapter
                .emit_object_event(*id, ObjectEvent::SelectionChanged);
        }
    }
}

impl TreeChangeHandler for AdapterChangeHandler<'_> {
    fn node_added(&mut self, node: &Node) {
        if filter(node) == FilterResult::Include {
            self.add_node(node);
        }
    }

    fn node_updated(&mut self, old_node: &Node, new_node: &Node) {
        self.emit_text_change_if_needed(old_node, new_node);
        let filter_old = filter(old_node);
        let filter_new = filter(new_node);
        if filter_new != filter_old {
            if filter_new == FilterResult::Include {
                if filter_old == FilterResult::ExcludeSubtree {
                    self.add_subtree(new_node);
                } else {
                    self.add_node(new_node);
                }
            } else if filter_old == FilterResult::Include {
                if filter_new == FilterResult::ExcludeSubtree {
                    self.hide_subtree(old_node, new_node.tree_state);
                } else {
                    self.notify_node(old_node, new_node);
                }
            }
        }
        self.notify_node(old_node, new_node);
        self.notify_parents(old_node, new_node);
        if filter_new == FilterResult::Include {
            self.emit_text_selection_change(Some(old_node), new_node);
        }
        if new_node.is_selected() != old_node.is_selected() {
            self.enqueue_selection_changed_if_needed(new_node);
        }
    }

    fn focus_moved(&mut self, old_node: Option<&Node>, new_node: Option<&Node>) {
        if let (None, Some(new_node)) = (old_node, new_node) {
            if let Some(root_window) = root_window(new_node.tree_state) {
                self.adapter.window_activated(&NodeWrapper(&root_window));
            }
        } else if let (Some(old_node), None) = (old_node, new_node) {
            if let Some(root_window) = root_window(old_node.tree_state) {
                self.adapter.window_deactivated(&NodeWrapper(&root_window));
            }
        }
        if let Some(node) = new_node {
            self.adapter
                .emit_object_event(node.id(), ObjectEvent::StateChanged(State::Focused, true));
            self.emit_text_selection_change(None, node);
        }
        if let Some(node) = old_node {
            self.adapter
                .emit_object_event(node.id(), ObjectEvent::StateChanged(State::Focused, false));
        }
    }

    fn node_removed(&mut self, node: &Node) {
        // Hidden nodes can still have registered client references.
        self.remove_node(node);
    }
}

static NEXT_ADAPTER_ID: AtomicUsize = AtomicUsize::new(0);

/// If you use this function, you must ensure that only one adapter at a time
/// has a given ID.
pub fn next_adapter_id() -> usize {
    NEXT_ADAPTER_ID.fetch_add(1, Ordering::Relaxed)
}

pub struct Adapter {
    id: usize,
    callback: Box<dyn AdapterCallback + Send + Sync>,
    context: Arc<Context>,
    registered_interfaces: RwLock<HashMap<NodeId, InterfaceSet>>,
}

impl Debug for Adapter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Adapter")
            .field("id", &self.id)
            .field("callback", &"AdapterCallback")
            .field("context", &self.context)
            .finish()
    }
}

impl Adapter {
    pub fn new(
        app_context: &Arc<RwLock<AppContext>>,
        callback: impl 'static + AdapterCallback + Send + Sync,
        initial_state: TreeUpdate,
        is_window_focused: bool,
        root_window_bounds: WindowBounds,
        action_handler: impl 'static + ActionHandler + Send,
    ) -> Self {
        let id = next_adapter_id();
        Self::with_id(
            id,
            app_context,
            callback,
            initial_state,
            is_window_focused,
            root_window_bounds,
            action_handler,
        )
    }

    pub fn with_id(
        id: usize,
        app_context: &Arc<RwLock<AppContext>>,
        callback: impl 'static + AdapterCallback + Send + Sync,
        initial_state: TreeUpdate,
        is_window_focused: bool,
        root_window_bounds: WindowBounds,
        action_handler: impl 'static + ActionHandler + Send,
    ) -> Self {
        Self::with_wrapped_action_handler(
            id,
            app_context,
            callback,
            initial_state,
            is_window_focused,
            root_window_bounds,
            Arc::new(ActionHandlerWrapper::new(action_handler)),
        )
    }

    /// This is an implementation detail of `accesskit_unix`, required for
    /// robust state transitions with minimal overhead.
    pub fn with_wrapped_action_handler(
        id: usize,
        app_context: &Arc<RwLock<AppContext>>,
        callback: impl 'static + AdapterCallback + Send + Sync,
        initial_state: TreeUpdate,
        is_window_focused: bool,
        root_window_bounds: WindowBounds,
        action_handler: Arc<dyn ActionHandlerNoMut + Send + Sync>,
    ) -> Self {
        let tree = Tree::new(initial_state, is_window_focused);
        let focus_id = tree.state().focus().map(|node| node.id());
        let context = Context::new(app_context, tree, action_handler, root_window_bounds);
        context.write_app_context().push_adapter(id, &context);
        let adapter = Self {
            id,
            callback: Box::new(callback),
            context,
            registered_interfaces: RwLock::new(HashMap::new()),
        };
        adapter.register_tree();
        if let Some(id) = focus_id {
            adapter.emit_object_event(id, ObjectEvent::StateChanged(State::Focused, true));
        }
        adapter
    }

    fn register_tree(&self) {
        fn add_children(node: Node<'_>, to_add: &mut Vec<(NodeId, InterfaceSet)>) {
            for child in node.filtered_children(&filter) {
                let child_id = child.id();
                let wrapper = NodeWrapper(&child);
                let interfaces = wrapper.interfaces();
                to_add.push((child_id, interfaces));
                add_children(child, to_add);
            }
        }

        let mut objects_to_add = Vec::new();

        let (adapter_index, root_id) = {
            let tree = self.context.read_tree();
            let tree_state = tree.state();
            let mut app_context = self.context.write_app_context();
            app_context.toolkit_name = Some(tree_state.toolkit_name().to_string());
            app_context.toolkit_version = tree_state.toolkit_version().map(|s| s.to_string());
            let adapter_index = app_context.adapter_index(self.id).unwrap();
            let root = tree_state.root();
            let root_id = root.id();
            let wrapper = NodeWrapper(&root);
            objects_to_add.push((root_id, wrapper.interfaces()));
            add_children(root, &mut objects_to_add);
            (adapter_index, root_id)
        };

        for (id, interfaces) in objects_to_add {
            self.sync_interfaces(id, interfaces);
            if id == root_id {
                self.window_created(adapter_index, id);
            }
        }
    }

    pub fn platform_node(&self, id: NodeId) -> PlatformNode {
        PlatformNode::new(&self.context, self.id, id)
    }

    pub fn root_id(&self) -> NodeId {
        self.context.read_tree().state().root_id()
    }

    pub fn platform_root(&self) -> PlatformRoot {
        PlatformRoot::new(&self.context.app_context)
    }

    fn is_registered(&self, id: NodeId) -> bool {
        self.registered_interfaces.read().unwrap().contains_key(&id)
    }

    fn sync_interfaces(&self, id: NodeId, desired: InterfaceSet) {
        let previous = self
            .registered_interfaces
            .write()
            .unwrap()
            .insert(id, desired)
            .unwrap_or_default();
        let shared = previous & desired;
        let removed = previous ^ shared;
        let added = desired ^ shared;
        if removed != InterfaceSet::empty() {
            self.callback.unregister_interfaces(self, id, removed);
        }
        if added != InterfaceSet::empty() {
            self.callback.register_interfaces(self, id, added);
        }
    }

    fn remove_interfaces(&self, id: NodeId) {
        let interfaces = self.registered_interfaces.write().unwrap().remove(&id);
        if let Some(interfaces) = interfaces {
            self.callback.unregister_interfaces(self, id, interfaces);
        }
    }

    pub(crate) fn emit_object_event(&self, target: NodeId, event: ObjectEvent) {
        let target = NodeIdOrRoot::Node(target);
        self.callback
            .emit_event(self, Event::Object { target, event });
    }

    fn emit_root_object_event(&self, event: ObjectEvent) {
        let target = NodeIdOrRoot::Root;
        self.callback
            .emit_event(self, Event::Object { target, event });
    }

    fn emit_cache_added(&self, target: NodeId) {
        self.callback
            .emit_event(self, Event::Cache(CacheEvent::Added(target)));
    }

    fn emit_cache_removed(&self, target: NodeId) {
        self.callback
            .emit_event(self, Event::Cache(CacheEvent::Removed(target)));
    }

    pub fn set_root_window_bounds(&mut self, new_bounds: WindowBounds) {
        let mut bounds = self.context.root_window_bounds.write().unwrap();
        *bounds = new_bounds;
    }

    pub fn update(&mut self, update: TreeUpdate) {
        let mut handler = AdapterChangeHandler::new(self);
        let mut tree = self.context.tree.write().unwrap();
        tree.update_and_process_changes(update, &mut handler);
        drop(tree);
        handler.emit_selection_changed();
    }

    pub fn update_window_focus_state(&mut self, is_focused: bool) {
        let mut handler = AdapterChangeHandler::new(self);
        let mut tree = self.context.tree.write().unwrap();
        tree.update_host_focus_state_and_process_changes(is_focused, &mut handler);
    }

    fn window_created(&self, adapter_index: usize, window: NodeId) {
        self.emit_root_object_event(ObjectEvent::ChildAdded(adapter_index, window));
    }

    fn window_activated(&self, window: &NodeWrapper<'_>) {
        self.callback.emit_event(
            self,
            Event::Window {
                target: window.id(),
                name: window.name().unwrap_or_default(),
                event: WindowEvent::Activated,
            },
        );
        self.emit_object_event(window.id(), ObjectEvent::StateChanged(State::Active, true));
        self.emit_root_object_event(ObjectEvent::ActiveDescendantChanged(window.id()));
    }

    fn window_deactivated(&self, window: &NodeWrapper<'_>) {
        self.callback.emit_event(
            self,
            Event::Window {
                target: window.id(),
                name: window.name().unwrap_or_default(),
                event: WindowEvent::Deactivated,
            },
        );
        self.emit_object_event(window.id(), ObjectEvent::StateChanged(State::Active, false));
    }

    fn window_destroyed(&self, window: NodeId) {
        self.emit_root_object_event(ObjectEvent::ChildRemoved(window));
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn is_window_focused(&self) -> bool {
        self.context.read_tree().state().is_host_focused()
    }

    pub fn root_window_bounds(&self) -> WindowBounds {
        *self.context.read_root_window_bounds()
    }

    /// This is an implementation detail of `accesskit_unix`, required for
    /// robust state transitions with minimal overhead.
    pub fn wrapped_action_handler(&self) -> Arc<dyn ActionHandlerNoMut + Send + Sync> {
        Arc::clone(&self.context.action_handler)
    }
}

fn root_window(current_state: &TreeState) -> Option<Node<'_>> {
    const WINDOW_ROLES: &[Role] = &[Role::AlertDialog, Role::Dialog, Role::Window];
    let root = current_state.root();
    if WINDOW_ROLES.contains(&root.role()) {
        Some(root)
    } else {
        None
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        let root_id = self.context.read_tree().state().root_id();
        self.window_destroyed(root_id);
        // Note: We deliberately do the following here, not in a Drop
        // implementation on context, because AppContext owns a second
        // strong reference to Context, and we need that to be released.
        self.context.write_app_context().remove_adapter(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::{Adapter, NodeIdOrRoot};
    use crate::{
        AdapterCallback, AppContext, CacheEvent, Event, InterfaceSet, ObjectEvent, WindowBounds,
    };
    use accesskit::{
        ActionHandler, ActionRequest, Node, NodeId as LocalNodeId, Role, Tree, TreeId, TreeUpdate,
    };
    use accesskit_consumer::NodeId;
    use atspi_common::{Role as AtspiRole, State};
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum CacheOp {
        Added(NodeId),
        Removed(NodeId),
    }

    type CapturedCache = Arc<Mutex<Vec<CacheOp>>>;
    type CapturedTrace = Arc<Mutex<Trace>>;

    struct CapturingCallback {
        ops: Arc<Mutex<Vec<CacheOp>>>,
        trace: Arc<Mutex<Trace>>,
    }

    #[derive(Default)]
    struct Trace {
        live: HashMap<NodeId, InterfaceSet>,
        registered: Vec<(NodeId, InterfaceSet)>,
        unregistered: Vec<(NodeId, InterfaceSet)>,
        states: Vec<(NodeId, State, bool)>,
        children: Vec<(NodeId, bool, NodeId)>,
    }

    impl Trace {
        fn clear_observations(&mut self) {
            self.registered.clear();
            self.unregistered.clear();
            self.states.clear();
            self.children.clear();
        }
    }

    impl AdapterCallback for CapturingCallback {
        fn register_interfaces(&self, _: &Adapter, id: NodeId, interfaces: InterfaceSet) {
            let mut trace = self.trace.lock().unwrap();
            let live = trace.live.entry(id).or_default();
            assert_eq!(
                *live & interfaces,
                InterfaceSet::empty(),
                "duplicate interface registration"
            );
            *live = *live | interfaces;
            trace.registered.push((id, interfaces));
        }
        fn unregister_interfaces(&self, _: &Adapter, id: NodeId, interfaces: InterfaceSet) {
            let mut trace = self.trace.lock().unwrap();
            let live = trace
                .live
                .get_mut(&id)
                .expect("unregistering an unknown logical node");
            assert_eq!(*live & interfaces, interfaces);
            *live = *live ^ interfaces;
            if *live == InterfaceSet::empty() {
                trace.live.remove(&id);
            }
            trace.unregistered.push((id, interfaces));
        }
        fn emit_event(&self, _: &Adapter, event: Event) {
            let mut ops = self.ops.lock().unwrap();
            match event {
                Event::Cache(CacheEvent::Added(id)) => ops.push(CacheOp::Added(id)),
                Event::Cache(CacheEvent::Removed(id)) => ops.push(CacheOp::Removed(id)),
                Event::Object {
                    target: NodeIdOrRoot::Node(id),
                    event: ObjectEvent::StateChanged(state, value),
                } => {
                    self.trace.lock().unwrap().states.push((id, state, value));
                }
                Event::Object {
                    target: NodeIdOrRoot::Node(id),
                    event: ObjectEvent::ChildAdded(_, child),
                } => {
                    self.trace.lock().unwrap().children.push((id, true, child));
                }
                Event::Object {
                    target: NodeIdOrRoot::Node(id),
                    event: ObjectEvent::ChildRemoved(child),
                } => {
                    self.trace.lock().unwrap().children.push((id, false, child));
                }
                _ => {}
            }
        }
    }

    struct NoOpActionHandler;
    impl ActionHandler for NoOpActionHandler {
        fn do_action(&mut self, _request: ActionRequest) {}
    }

    fn with_children(role: Role, children: &[LocalNodeId]) -> Node {
        let mut node = Node::new(role);
        node.set_children(children.to_vec());
        node
    }

    fn build(initial: TreeUpdate) -> (Adapter, Arc<Mutex<Vec<CacheOp>>>) {
        let (adapter, ops, _) = build_with_trace(initial);
        (adapter, ops)
    }

    fn build_with_trace(initial: TreeUpdate) -> (Adapter, CapturedCache, CapturedTrace) {
        let ops = Arc::new(Mutex::new(Vec::new()));
        let trace = Arc::new(Mutex::new(Trace::default()));
        let app_context = AppContext::new(None);
        let adapter = Adapter::new(
            &app_context,
            CapturingCallback {
                ops: ops.clone(),
                trace: trace.clone(),
            },
            initial,
            false,
            WindowBounds::default(),
            NoOpActionHandler,
        );
        (adapter, ops, trace)
    }

    fn initial_tree() -> TreeUpdate {
        TreeUpdate {
            nodes: vec![
                (
                    LocalNodeId(0),
                    with_children(Role::Window, &[LocalNodeId(1)]),
                ),
                (LocalNodeId(1), Node::new(Role::Button)),
            ],
            tree: Some(Tree::new(LocalNodeId(0))),
            tree_id: TreeId::ROOT,
            focus: LocalNodeId(0),
        }
    }

    fn update(nodes: Vec<(LocalNodeId, Node)>) -> TreeUpdate {
        TreeUpdate {
            nodes,
            tree: None,
            tree_id: TreeId::ROOT,
            focus: LocalNodeId(0),
        }
    }

    #[test]
    fn no_cache_events_on_construction() {
        let (_adapter, ops) = build(initial_tree());
        assert!(ops.lock().unwrap().is_empty());
    }

    #[test]
    fn add_node_emits_one_added() {
        let (mut adapter, ops) = build(initial_tree());
        ops.lock().unwrap().clear();
        adapter.update(update(vec![
            (
                LocalNodeId(0),
                with_children(Role::Window, &[LocalNodeId(1), LocalNodeId(2)]),
            ),
            (LocalNodeId(2), Node::new(Role::Button)),
        ]));
        let ops = ops.lock().unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], CacheOp::Added(_)));
    }

    #[test]
    fn remove_node_emits_removed_for_same_id() {
        let (mut adapter, ops) = build(initial_tree());
        adapter.update(update(vec![
            (
                LocalNodeId(0),
                with_children(Role::Window, &[LocalNodeId(1), LocalNodeId(2)]),
            ),
            (LocalNodeId(2), Node::new(Role::Button)),
        ]));
        let added_id = match ops.lock().unwrap().as_slice() {
            [CacheOp::Added(id)] => *id,
            other => panic!("expected exactly one Added, got {other:?}"),
        };
        ops.lock().unwrap().clear();
        adapter.update(update(vec![(
            LocalNodeId(0),
            with_children(Role::Window, &[LocalNodeId(1)]),
        )]));
        assert_eq!(*ops.lock().unwrap(), vec![CacheOp::Removed(added_id)]);
    }

    #[test]
    fn subtree_add_emits_added_per_node() {
        let (mut adapter, ops) = build(initial_tree());
        ops.lock().unwrap().clear();
        adapter.update(update(vec![
            (
                LocalNodeId(0),
                with_children(Role::Window, &[LocalNodeId(1), LocalNodeId(2)]),
            ),
            (
                LocalNodeId(2),
                with_children(Role::Group, &[LocalNodeId(3), LocalNodeId(4)]),
            ),
            (LocalNodeId(3), Node::new(Role::Button)),
            (LocalNodeId(4), Node::new(Role::Button)),
        ]));
        let ops = ops.lock().unwrap();
        assert_eq!(ops.len(), 3);
        assert!(ops.iter().all(|op| matches!(op, CacheOp::Added(_))));
    }

    #[test]
    fn subtree_remove_emits_removed_per_node() {
        let (mut adapter, ops) = build(initial_tree());
        adapter.update(update(vec![
            (
                LocalNodeId(0),
                with_children(Role::Window, &[LocalNodeId(1), LocalNodeId(2)]),
            ),
            (
                LocalNodeId(2),
                with_children(Role::Group, &[LocalNodeId(3), LocalNodeId(4)]),
            ),
            (LocalNodeId(3), Node::new(Role::Button)),
            (LocalNodeId(4), Node::new(Role::Button)),
        ]));
        ops.lock().unwrap().clear();
        adapter.update(update(vec![(
            LocalNodeId(0),
            with_children(Role::Window, &[LocalNodeId(1)]),
        )]));
        let ops = ops.lock().unwrap();
        assert_eq!(ops.len(), 3);
        assert!(ops.iter().all(|op| matches!(op, CacheOp::Removed(_))));
    }

    #[test]
    fn filter_transition_into_tree_emits_added() {
        let mut hidden = Node::new(Role::Button);
        hidden.set_hidden();
        let (mut adapter, ops) = build(TreeUpdate {
            nodes: vec![
                (
                    LocalNodeId(0),
                    with_children(Role::Window, &[LocalNodeId(1), LocalNodeId(2)]),
                ),
                (LocalNodeId(1), Node::new(Role::Button)),
                (LocalNodeId(2), hidden),
            ],
            tree: Some(Tree::new(LocalNodeId(0))),
            tree_id: TreeId::ROOT,
            focus: LocalNodeId(0),
        });
        ops.lock().unwrap().clear();
        adapter.update(update(vec![(LocalNodeId(2), Node::new(Role::Button))]));
        let ops = ops.lock().unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], CacheOp::Added(_)));
    }

    #[test]
    fn filter_transition_out_of_tree_emits_removed() {
        let (mut adapter, ops, trace) = build_with_trace(initial_tree());
        let root_id = adapter.root_id();
        let child_id = adapter
            .platform_node(root_id)
            .child_at_index(0)
            .unwrap()
            .unwrap();
        trace.lock().unwrap().clear_observations();
        ops.lock().unwrap().clear();
        let mut hidden = Node::new(Role::Button);
        hidden.set_hidden();
        adapter.update(update(vec![(LocalNodeId(1), hidden)]));
        let ops = ops.lock().unwrap();
        assert!(
            ops.is_empty(),
            "filtering must not dispose the client object"
        );
        let trace = trace.lock().unwrap();
        assert_eq!(trace.children, vec![(root_id, false, child_id)]);
        assert!(trace.states.contains(&(child_id, State::Showing, false)));
        assert!(
            !trace
                .states
                .iter()
                .any(|(_, state, _)| *state == State::Defunct)
        );
    }

    #[test]
    fn repeated_filtering_retains_references_and_does_not_duplicate_interfaces() {
        let (mut adapter, ops, trace) = build_with_trace(initial_tree());
        let root = adapter.platform_node(adapter.root_id());
        let child_id = root.child_at_index(0).unwrap().unwrap();
        let retained = adapter.platform_node(child_id);
        trace.lock().unwrap().clear_observations();
        for _ in 0..8 {
            let mut hidden = Node::new(Role::Button);
            hidden.set_hidden();
            adapter.update(update(vec![(LocalNodeId(1), hidden)]));
            assert_eq!(root.child_count().unwrap(), 0);
            assert_eq!(retained.role().unwrap(), AtspiRole::Button);
            let state = retained.state();
            assert!(!state.contains(State::Defunct));
            assert!(!state.contains(State::Showing));
            assert!(!state.contains(State::Visible));
            adapter.update(update(vec![(LocalNodeId(1), Node::new(Role::Button))]));
            assert_eq!(root.child_at_index(0).unwrap(), Some(child_id));
            assert!(retained.state().contains(State::Showing | State::Visible));
        }
        assert_eq!(*ops.lock().unwrap(), vec![CacheOp::Added(child_id); 8]);
        let trace = trace.lock().unwrap();
        assert!(trace.registered.is_empty() && trace.unregistered.is_empty());
        assert_eq!(
            trace.children.iter().filter(|(_, added, _)| *added).count(),
            8
        );
        assert_eq!(
            trace
                .children
                .iter()
                .filter(|(_, added, _)| !*added)
                .count(),
            8
        );
        assert!(
            !trace
                .states
                .iter()
                .any(|(_, state, _)| *state == State::Defunct)
        );
    }

    #[test]
    fn hidden_subtree_reappears_then_actual_hidden_removal_disposes_every_node() {
        let (mut adapter, ops, trace) = build_with_trace(TreeUpdate {
            nodes: vec![
                (
                    LocalNodeId(0),
                    with_children(Role::Window, &[LocalNodeId(1)]),
                ),
                (
                    LocalNodeId(1),
                    with_children(Role::Group, &[LocalNodeId(2)]),
                ),
                (LocalNodeId(2), Node::new(Role::Button)),
            ],
            tree: Some(Tree::new(LocalNodeId(0))),
            tree_id: TreeId::ROOT,
            focus: LocalNodeId(0),
        });
        let root = adapter.platform_node(adapter.root_id());
        let group_id = root.child_at_index(0).unwrap().unwrap();
        let group = adapter.platform_node(group_id);
        let child_id = group.child_at_index(0).unwrap().unwrap();
        let child = adapter.platform_node(child_id);
        trace.lock().unwrap().clear_observations();
        let hide = || {
            let mut node = with_children(Role::Group, &[LocalNodeId(2)]);
            node.set_hidden();
            node
        };
        adapter.update(update(vec![(LocalNodeId(1), hide())]));
        assert_eq!(root.child_count().unwrap(), 0);
        assert!(!child.state().contains(State::Defunct));
        assert!(!child.state().contains(State::Showing));
        adapter.update(update(vec![(
            LocalNodeId(1),
            with_children(Role::Group, &[LocalNodeId(2)]),
        )]));
        assert_eq!(root.child_at_index(0).unwrap(), Some(group_id));
        assert_eq!(group.child_at_index(0).unwrap(), Some(child_id));
        assert!(child.state().contains(State::Showing));
        adapter.update(update(vec![(LocalNodeId(1), hide())]));
        ops.lock().unwrap().clear();
        adapter.update(update(vec![(
            LocalNodeId(0),
            with_children(Role::Window, &[]),
        )]));
        assert!(group.state().contains(State::Defunct) && child.state().contains(State::Defunct));
        assert!(group.role().is_err() && child.role().is_err());
        let removed = ops.lock().unwrap();
        assert_eq!(removed.len(), 2);
        assert!(
            removed.contains(&CacheOp::Removed(group_id))
                && removed.contains(&CacheOp::Removed(child_id))
        );
        let trace = trace.lock().unwrap();
        assert!(!trace.live.contains_key(&group_id) && !trace.live.contains_key(&child_id));
        assert_eq!(
            trace
                .states
                .iter()
                .filter(|(_, state, value)| *state == State::Defunct && *value)
                .count(),
            2
        );
    }

    #[test]
    fn hidden_role_change_reconciles_interfaces_and_unexported_removal_is_silent() {
        let (mut adapter, ops, trace) = build_with_trace(initial_tree());
        let root = adapter.platform_node(adapter.root_id());
        let child_id = root.child_at_index(0).unwrap().unwrap();
        let retained = adapter.platform_node(child_id);
        let mut hidden = Node::new(Role::TextInput);
        hidden.set_hidden();
        adapter.update(update(vec![(LocalNodeId(1), hidden)]));
        assert_eq!(retained.role().unwrap(), AtspiRole::Entry);
        assert!(!retained.state().contains(State::Defunct));
        adapter.update(update(vec![(
            LocalNodeId(0),
            with_children(Role::Window, &[]),
        )]));
        assert!(!trace.lock().unwrap().live.contains_key(&child_id));
        assert_eq!(*ops.lock().unwrap(), vec![CacheOp::Removed(child_id)]);

        let mut hidden = Node::new(Role::Button);
        hidden.set_hidden();
        adapter.update(update(vec![
            (
                LocalNodeId(0),
                with_children(Role::Window, &[LocalNodeId(2)]),
            ),
            (LocalNodeId(2), hidden),
        ]));
        ops.lock().unwrap().clear();
        trace.lock().unwrap().clear_observations();
        adapter.update(update(vec![(
            LocalNodeId(0),
            with_children(Role::Window, &[]),
        )]));
        assert!(ops.lock().unwrap().is_empty());
        assert!(trace.lock().unwrap().unregistered.is_empty());
    }
}
