//! Context menu and hover card shared by every view that shows entries.

use egui::{Layout, Ui};
use storagemap_core::{Flags, Kind, Metric, NodeId, Tree};

use super::Env;
use crate::app::Action;
use crate::fmt;

pub fn show(ui: &mut Ui, env: &mut Env<'_>, tree: &Tree, id: NodeId) {
    let dir = env.i18n.dir();
    ui.with_layout(Layout::top_down_justified(dir.start()), |ui| {
        let node = tree.node(id);
        let mut item = |ui: &mut Ui, label: &str, action: Action| {
            let icon = match label {
                "action-scan-this" => crate::icons::FOLDER_OPEN,
                "action-open" => crate::icons::OPEN,
                "action-show-in-folder" => crate::icons::REVEAL,
                "action-copy-path" => crate::icons::COPY,
                "action-trash" | "action-empty-trash" => crate::icons::TRASH,
                _ => "",
            };
            if ui.button(format!("{icon}  {}", env.t(label))).clicked() {
                env.push(action);
                ui.close();
            }
        };
        if node.is_dir() {
            item(ui, "action-scan-this", Action::ScanSubtree(id));
        }
        item(ui, "action-open", Action::Open(id));
        item(ui, "action-show-in-folder", Action::Reveal(id));
        item(ui, "action-copy-path", Action::CopyPath(id));
        match crate::platform::trash_location(&tree.path(id)) {
            crate::platform::TrashLocation::Root => {
                ui.separator();
                item(ui, "action-empty-trash", Action::AskEmptyTrash(id));
            }
            // Already in the trash: nothing to move.
            crate::platform::TrashLocation::Inside => {}
            crate::platform::TrashLocation::Outside if id != tree.root() => {
                ui.separator();
                item(ui, "action-trash", Action::AskTrash(id));
            }
            crate::platform::TrashLocation::Outside => {}
        }
    });
}

/// Hover card: name, size, share, counts and any scan notes.
pub fn tooltip(ui: &mut Ui, env: &Env<'_>, tree: &Tree, id: NodeId) {
    let i18n = env.i18n;
    let node = tree.node(id);
    ui.with_layout(Layout::top_down(i18n.dir().start()), |ui| {
        ui.strong(i18n.v(&tree.name(id)));
        let share = fmt::percent(tree.share_of_parent(id, env.settings.metric));
        let line = format!(
            "{} · {}",
            env.size(node.size),
            i18n.logical("metric-allocated") + ": " + &env.size(node.allocated)
        );
        ui.label(i18n.v(&line));
        ui.label(i18n.ta("percent-of-parent", &[("percent", share.into())]));
        if node.kind == Kind::Dir {
            let counts = format!(
                "{} · {}",
                count_text(env, "files-count", node.files),
                count_text(env, "folders-count", node.dirs)
            );
            ui.label(i18n.v(&counts));
        }
        if node.modified > 0 {
            ui.label(i18n.v(&format!(
                "{}: {}",
                i18n.logical("col-modified"),
                fmt::date(node.modified)
            )));
        }
        let attributes: Vec<String> = [
            (Flags::READ_ONLY, "attr-read-only"),
            (Flags::HIDDEN, "attr-hidden"),
            (Flags::SYSTEM, "attr-system"),
            (Flags::COMPRESSED, "attr-compressed"),
            (Flags::ENCRYPTED, "attr-encrypted"),
            (Flags::SPARSE, "attr-sparse"),
        ]
        .into_iter()
        .filter(|&(flag, _)| node.flags.contains(flag))
        .map(|(_, id)| i18n.logical(id))
        .collect();
        if !attributes.is_empty() {
            ui.label(i18n.v(&format!(
                "{}: {}",
                i18n.logical("col-attributes"),
                attributes.join(", ")
            )));
        }
        for (flag, id) in [
            (Flags::HARDLINK_DUP, "note-hardlink"),
            (Flags::MOUNT_POINT, "note-mount"),
            (Flags::ERROR, "note-error"),
            (Flags::INCOMPLETE, "note-incomplete"),
        ] {
            if node.flags.contains(flag) {
                ui.colored_label(ui.visuals().warn_fg_color, i18n.t(id));
            }
        }
        if env.settings.metric != Metric::Size {
            ui.weak(i18n.v(&env.metric_text(node)));
        }
    });
}

/// Plural-aware count in logical order.
fn count_text(env: &Env<'_>, id: &str, n: u64) -> String {
    env.i18n.count(id, n)
}
