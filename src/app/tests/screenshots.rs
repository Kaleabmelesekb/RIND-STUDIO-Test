//! Not a check: the running app's look, rendered in software to PNGs, so it can be looked at (and compared with the
//! design) without a phone. `SHOT_DIR=<dir> cargo test --lib render_screens -- --ignored`; the `Screenshots` workflow
//! runs it and uploads the images.

use std::path::Path;

use super::*;
use crate::content::demo_songs;
use crate::ui::screenshot::write_png;
use crate::ui::split::DockTab;
use crate::ui::views::View;

fn shoot(rig: &mut Rig, dir: &Path, name: &str) {
    for _ in 0..6 {
        rig.frame(vec![]);
    }
    let rgb = rig.textures.paint(&rig.ctx, &rig.out, 1170, 540);
    write_png(&dir.join(format!("{name}.png")), &rgb, 1170, 540);
}

#[test]
#[ignore]
fn render_screens() {
    let dir = std::path::PathBuf::from(std::env::var("SHOT_DIR").expect("set SHOT_DIR"));
    std::fs::create_dir_all(&dir).expect("the folder");
    crate::ui::tokens::screenshots::set(true);

    let mut rig = Rig::new();
    let (_, bytes) = demo_songs().remove(0);
    let song = crate::flm::read(&bytes).expect("the demo song");
    let files = without_factory(ProjectFiles::with_provider(Box::new(MemFiles::default())));
    rig.app = HoneydewApp {
        files,
        ..HoneydewApp::with_song(song)
    };
    rig.frame(vec![]);
    rig.app.navigate(Nav::Close);

    shoot(&mut rig, &dir, "1-playlist");

    rig.app.main_screen.dock = DockTab::Devices;
    rig.app.main_screen.dock_open = true;
    shoot(&mut rig, &dir, "2-dock-devices");

    rig.app.main_screen.dock = DockTab::Mixer;
    shoot(&mut rig, &dir, "3-dock-mixer");

    rig.app.main_screen.dock = DockTab::Clip;
    rig.app.navigate(Nav::Edit(rig.app.selected_channel));
    shoot(&mut rig, &dir, "4-dock-clip");

    rig.app.navigate(Nav::Collapse);
    rig.app.main_screen.dock_open = false;
    rig.app.navigate(Nav::Show(View::Mixer));
    shoot(&mut rig, &dir, "5-mixer");

    crate::ui::tokens::screenshots::set(false);
}
