// SPDX-License-Identifier: GPL-3.0-or-later
use nih_plug::prelude::*;
use std::sync::{Arc, Mutex};
#[derive(Default)]
pub struct Edits(Mutex<Vec<String>>);
impl Edits {
    pub fn take(&self) -> Vec<String> { self.0.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default() }
}
struct Context { inner: Arc<dyn GuiContext>, edits: Arc<Edits> }
pub fn wrap(inner: Arc<dyn GuiContext>, edits: Arc<Edits>) -> Arc<dyn GuiContext> { Arc::new(Context { inner, edits }) }
pub fn difference(before: &PluginState, after: &PluginState) -> Option<String> {
    let mut patch = serde_json::Map::new();
    // Compare persistent strings directly: unchanged embedded audio/tables are
    // never copied or serialized just to discover that only a parameter moved.
    let keys: std::collections::BTreeSet<_> = before.fields.keys().chain(after.fields.keys()).collect();
    let changes: serde_json::Map<String, serde_json::Value> = keys.into_iter()
        .filter(|key| before.fields.get(*key) != after.fields.get(*key))
        .map(|key| (key.clone(), before.fields.get(key).map_or(serde_json::Value::Null, |v| serde_json::Value::String(v.clone())))).collect();
    if !changes.is_empty() { patch.insert("fields".into(), serde_json::Value::Object(changes)); }
    let keys: std::collections::BTreeSet<_> = before.params.keys().chain(after.params.keys()).collect();
    let mut changes = serde_json::Map::new();
    for key in keys {
        let old = serde_json::to_value(before.params.get(key)).ok()?;
        if old != serde_json::to_value(after.params.get(key)).ok()? { changes.insert(key.clone(), old); }
    }
    if !changes.is_empty() { patch.insert("params".into(), serde_json::Value::Object(changes)); }
    (!patch.is_empty()).then(|| serde_json::Value::Object(patch).to_string())
}
impl GuiContext for Context {
    fn plugin_api(&self) -> PluginApi { self.inner.plugin_api() }
    fn request_resize(&self) -> bool { self.inner.request_resize() }
    unsafe fn raw_begin_set_parameter(&self, p: ParamPtr) { unsafe { self.inner.raw_begin_set_parameter(p) } }
    unsafe fn raw_set_parameter_normalized(&self, p: ParamPtr, v: f32) { unsafe { self.inner.raw_set_parameter_normalized(p, v) } }
    unsafe fn raw_end_set_parameter(&self, p: ParamPtr) { unsafe { self.inner.raw_end_set_parameter(p) } }
    fn get_state(&self) -> PluginState { self.inner.get_state() }
    fn set_state(&self, state: PluginState) {
        let patch = difference(&self.inner.get_state(), &state);
        self.inner.set_state(state);
        if let Some(patch) = patch {
            if let Ok(mut queue) = self.edits.0.lock() {
                if patch.len() <= os_hostlink::STATE_EDIT_BUDGET {
                    while !queue.is_empty() && (queue.len() >= 128 || queue.iter().map(String::len).sum::<usize>() + patch.len() > os_hostlink::STATE_EDIT_BUDGET) { queue.remove(0); }
                    queue.push(patch);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::wrapper::state::ParamValue;
    struct Mock(Mutex<PluginState>);
    impl GuiContext for Mock {
        fn plugin_api(&self) -> PluginApi { PluginApi::Clap }
        fn request_resize(&self) -> bool { true }
        unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
        unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
        fn get_state(&self) -> PluginState { self.0.lock().unwrap().clone() }
        fn set_state(&self, state: PluginState) { *self.0.lock().unwrap() = state; }
    }
    #[test]
    fn native_preset_undo_captures_changes_without_unchanged_assets() {
        let initial = PluginState { version: "1".into(), params: [("gain".into(), ParamValue::F32(0.5))].into(),
            fields: [("large-table".into(), "x".repeat(1024 * 1024)), ("curve".into(), "old".into())].into() };
        let raw = Arc::new(Mock(Mutex::new(initial.clone())));
        let edits = Arc::new(Edits::default());
        let context = wrap(raw.clone(), edits.clone());
        context.set_state(initial.clone());
        assert!(edits.take().is_empty(), "no-op preset creates no undo step");
        let mut next = initial;
        next.params.insert("gain".into(), ParamValue::F32(0.75));
        next.fields.insert("curve".into(), "new".into());
        next.fields.insert("added".into(), "value".into());
        context.set_state(next);
        let patches = edits.take();
        assert_eq!(patches.len(), 1);
        assert!(patches[0].len() < 200, "unchanged embedded assets are not retained");
        let patch: serde_json::Value = serde_json::from_str(&patches[0]).unwrap();
        assert_eq!(patch["fields"]["curve"], "old");
        assert!(patch["fields"]["added"].is_null());
        assert_eq!(patch["params"]["gain"]["f32"], 0.5);
        let mut external = raw.get_state(); external.fields.insert("curve".into(), "host undo".into());
        raw.set_state(external);
        assert!(edits.take().is_empty(), "host loads bypass the editor transaction wrapper");
    }
}
