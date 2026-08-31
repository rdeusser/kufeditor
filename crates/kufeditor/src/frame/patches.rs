use std::path::Path;

use gpui::{Context, Window};
use kufeditor_game::Game;
use kufeditor_patches::{
    FireRatePresetID, PatchChange, PatchError, PatchID, PatchOperationResult, PatchTarget, inspect,
    set_fire_rate, set_patch,
};

use super::AppFrame;
use crate::{
    actions::{DismissPatchConfirmation, FocusNextPatchControl, FocusPreviousPatchControl},
    notices::{Notice, NoticeSource, format_error},
    patch_status::{
        PatchContextChange, PatchFinish, PatchKey, PatchOperation, PatchOperationKey, PatchSnapshot,
    },
    state::Area,
};

impl AppFrame {
    pub(super) fn focus_next_patch_control(
        &mut self,
        _: &FocusNextPatchControl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.area() == Area::Patches {
            window.focus_next();
            cx.notify();
        }
    }

    pub(super) fn focus_previous_patch_control(
        &mut self,
        _: &FocusPreviousPatchControl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.area() == Area::Patches {
            window.focus_prev();
            cx.notify();
        }
    }

    pub(super) fn dismiss_patch_confirmation_action(
        &mut self,
        _: &DismissPatchConfirmation,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.area() == Area::Patches {
            self.dismiss_patch_confirmation(cx);
        }
    }

    pub(crate) fn start_patch_inspection(&mut self, cx: &mut Context<Self>) {
        self.sync_patch_context();
        let Some(key) = self.patches.begin_inspection() else {
            cx.notify();
            return;
        };
        let Some(root) = key.root().map(Path::to_path_buf) else {
            cx.notify();
            return;
        };
        let game = key.game();
        #[cfg(test)]
        {
            self.task_launches.patch_inspections += 1;
        }
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { inspect(game, &root) });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_patch_inspection(key, result, cx);
            });
        })
        .detach();
    }

    pub(crate) fn request_patch_change(
        &mut self,
        id: PatchID,
        target: PatchTarget,
        cx: &mut Context<Self>,
    ) {
        if self
            .patches
            .request_operation(PatchOperation::SetPatch { id, target })
        {
            cx.notify();
        }
    }

    pub(crate) fn request_fire_rate_change(
        &mut self,
        id: FireRatePresetID,
        cx: &mut Context<Self>,
    ) {
        if self
            .patches
            .request_operation(PatchOperation::SetFireRate { id })
        {
            cx.notify();
        }
    }

    pub(crate) fn dismiss_patch_confirmation(&mut self, cx: &mut Context<Self>) {
        self.patches.dismiss_confirmation();
        cx.notify();
    }

    pub(crate) fn confirm_patch_operation(&mut self, cx: &mut Context<Self>) {
        let Some(launch) = self.patches.confirm_operation() else {
            return;
        };
        let Some(root) = launch.root().map(Path::to_path_buf) else {
            self.finish_patch_operation(
                launch.key(),
                Err(PatchError::UnsupportedGame { game: Game::Heroes }),
                cx,
            );
            return;
        };
        let key = launch.key().clone();
        let operation = launch.operation();
        self.notices.begin(
            NoticeSource::Patches,
            key.request().get(),
            Notice::info(format!(
                "{} in progress",
                operation_progress_label(operation)
            )),
        );
        #[cfg(test)]
        {
            self.task_launches.patch_operations += 1;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            match operation {
                PatchOperation::SetPatch { id, target } => {
                    set_patch(Game::Crusaders, &root, id, target)
                }
                PatchOperation::SetFireRate { id } => set_fire_rate(Game::Crusaders, &root, id),
            }
        });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_patch_operation(&key, result, cx);
            });
        })
        .detach();
    }

    pub(super) fn active_patch_context_changed(&mut self, cx: &mut Context<Self>) {
        if self.sync_patch_context() == PatchContextChange::Changed {
            self.notices.clear(NoticeSource::Patches);
            if self.shell.area() == Area::Patches && !self.patches.operation_in_progress() {
                self.start_patch_inspection(cx);
            }
        }
    }

    fn sync_patch_context(&mut self) -> PatchContextChange {
        let game = self.shell.game();
        self.patches.set_context(
            game,
            self.game_paths.root(game).map(Path::to_path_buf),
            self.root_revisions.revision(game),
        )
    }

    fn finish_patch_inspection(
        &mut self,
        key: PatchKey,
        result: Result<kufeditor_patches::ExecutableInspection, PatchError>,
        cx: &mut Context<Self>,
    ) {
        let result = result
            .map(|inspection| PatchSnapshot::from_inspection(&inspection))
            .map_err(|error| format_error(&error));
        if self.patches.finish_inspection(key, result) {
            cx.notify();
        }
    }

    fn finish_patch_operation(
        &mut self,
        key: &PatchOperationKey,
        result: Result<PatchOperationResult, PatchError>,
        cx: &mut Context<Self>,
    ) {
        match self.patches.finish_operation(key) {
            PatchFinish::Ignored => {}
            PatchFinish::ContextChanged => {
                if self.shell.area() == Area::Patches {
                    self.start_patch_inspection(cx);
                } else {
                    cx.notify();
                }
            }
            PatchFinish::Current => {
                let notice = match result {
                    Ok(result) => {
                        Notice::success(operation_success_message(key.operation(), result))
                    }
                    Err(error) => Notice::error(
                        format!("{} failed", operation_progress_label(key.operation())),
                        &error,
                    ),
                };
                let completed =
                    self.notices
                        .complete(NoticeSource::Patches, key.request().get(), Some(notice));
                if completed {
                    self.schedule_success_notice_dismissal(NoticeSource::Patches, cx);
                }
                self.start_patch_inspection(cx);
            }
        }
    }
}

const fn operation_progress_label(operation: PatchOperation) -> &'static str {
    match operation {
        PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            target: PatchTarget::Applied,
        } => "Applying Debug Menu",
        PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            target: PatchTarget::NotApplied,
        } => "Reverting Debug Menu",
        PatchOperation::SetPatch {
            id: PatchID::TerrainBounds,
            target: PatchTarget::Applied,
        } => "Applying Terrain Bounds Check",
        PatchOperation::SetPatch {
            id: PatchID::TerrainBounds,
            target: PatchTarget::NotApplied,
        } => "Reverting Terrain Bounds Check",
        PatchOperation::SetFireRate { .. } => "Changing fire rate",
    }
}

fn operation_success_message(operation: PatchOperation, result: PatchOperationResult) -> String {
    if result.change() == PatchChange::Unchanged {
        return format!("{} was already current", operation_subject(operation));
    }
    match operation {
        PatchOperation::SetPatch {
            target: PatchTarget::Applied,
            ..
        } => format!("{} applied", operation_subject(operation)),
        PatchOperation::SetPatch {
            target: PatchTarget::NotApplied,
            ..
        } => format!("{} reverted", operation_subject(operation)),
        PatchOperation::SetFireRate { .. } => {
            format!("{} selected", operation_subject(operation))
        }
    }
}

const fn operation_subject(operation: PatchOperation) -> &'static str {
    match operation {
        PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            ..
        } => "Debug Menu",
        PatchOperation::SetPatch {
            id: PatchID::TerrainBounds,
            ..
        } => "Terrain Bounds Check",
        PatchOperation::SetFireRate {
            id: FireRatePresetID::Original,
        } => "Original fire rate",
        PatchOperation::SetFireRate {
            id: FireRatePresetID::Fast,
        } => "Fast fire rate",
        PatchOperation::SetFireRate {
            id: FireRatePresetID::Rapid,
        } => "Rapid fire rate",
        PatchOperation::SetFireRate {
            id: FireRatePresetID::Turbo,
        } => "Turbo fire rate",
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "the GPUI tests use controlled temporary executables and windows"
    )]

    use std::{
        fs::{self, File},
        io::{Seek, SeekFrom, Write},
        path::{Path, PathBuf},
    };

    use gpui::{
        AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, WindowOptions, point, px,
        size,
    };
    use kufeditor_game::Game;
    use kufeditor_patches::{PatchID, PatchStatus, PatchTarget};
    use tempfile::TempDir;

    use super::super::AppFrame;
    use crate::{
        notices::Notice, patch_status::PatchInspectionPhase, settings::SettingsStartup, state::Area,
    };

    #[gpui::test]
    fn patches_route_inspects_the_exact_configured_executable(cx: &mut TestAppContext) {
        let fixture = ExecutableFixture::original();
        let window = test_window(cx, fixture.root());

        window
            .update(cx, |frame, _, cx| {
                frame.select_area(Area::Patches, cx);
                assert!(matches!(
                    frame.patches.phase(),
                    PatchInspectionPhase::Loading { .. }
                ));
                assert_eq!(frame.task_launches.patch_inspections, 1);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                let PatchInspectionPhase::Ready { snapshot, .. } = frame.patches.phase() else {
                    panic!("expected ready patch inspection");
                };
                assert_eq!(snapshot.executable(), fixture.path());
                assert_eq!(
                    snapshot.patch_status(PatchID::DebugMenu),
                    PatchStatus::NotApplied
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn confirmed_patch_operation_refreshes_the_ready_state(cx: &mut TestAppContext) {
        let fixture = ExecutableFixture::original();
        let window = test_window(cx, fixture.root());
        window
            .update(cx, |frame, _, cx| frame.select_area(Area::Patches, cx))
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, cx| {
                frame.request_patch_change(PatchID::DebugMenu, PatchTarget::Applied, cx);
                assert!(frame.patches.pending_confirmation().is_some());
                frame.confirm_patch_operation(cx);
                assert!(frame.patches.operation_in_progress());
                assert_eq!(frame.task_launches.patch_operations, 1);
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Applying Debug Menu in progress")
                );
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                let PatchInspectionPhase::Ready { snapshot, .. } = frame.patches.phase() else {
                    panic!("expected refreshed patch inspection");
                };
                assert_eq!(
                    snapshot.patch_status(PatchID::DebugMenu),
                    PatchStatus::Applied
                );
                assert!(!frame.patches.operation_in_progress());
                assert_eq!(frame.task_launches.patch_inspections, 2);
                assert!(fixture.backup_path().is_file());
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Debug Menu applied")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn game_change_rejects_the_old_inspection_completion(cx: &mut TestAppContext) {
        let fixture = ExecutableFixture::original();
        let window = test_window(cx, fixture.root());

        window
            .update(cx, |frame, _, cx| {
                frame.select_area(Area::Patches, cx);
                frame.select_game(Game::Heroes, cx);
                assert_eq!(frame.patches.game(), Game::Heroes);
                assert!(matches!(frame.patches.phase(), PatchInspectionPhase::Idle));
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.patches.game(), Game::Heroes);
                assert!(matches!(frame.patches.phase(), PatchInspectionPhase::Idle));
            })
            .unwrap();
    }

    #[gpui::test]
    fn patches_pointer_and_keyboard_controls_share_the_confirmation_flow(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let fixture = ExecutableFixture::original();
        let startup = test_startup(fixture.root());
        let (frame, cx) = cx.add_window_view(move |_, cx| AppFrame::new(startup, cx));
        frame.update(cx, |frame, cx| frame.select_area(Area::Patches, cx));
        cx.run_until_parked();
        draw_patch_frame(cx, &frame);

        for selector in [
            "patches-route",
            "patch-refresh",
            "patch-debug-menu-action",
            "patch-terrain-bounds-action",
            "patch-fire-original",
            "patch-fire-fast",
            "patch-fire-rapid",
            "patch-fire-turbo",
        ] {
            assert!(
                cx.debug_bounds(selector).is_some(),
                "missing Patches control {selector}"
            );
        }

        click(cx, "patch-debug-menu-action");
        frame.update(cx, |frame, _| {
            assert!(frame.patches.pending_confirmation().is_some());
        });
        draw_patch_frame(cx, &frame);
        click(cx, "patch-confirmation-dismiss");
        frame.update(cx, |frame, _| {
            assert!(frame.patches.pending_confirmation().is_none());
        });

        draw_patch_frame(cx, &frame);
        click(cx, "patch-debug-menu-action");
        draw_patch_frame(cx, &frame);
        frame.update_in(cx, |frame, window, _| window.focus(&frame.patches_focus));
        cx.simulate_keystrokes("tab tab");
        frame.update_in(cx, |frame, window, _| {
            assert!(
                !frame.patches_focus.is_focused(window),
                "Tab did not move focus away from the tracked Refresh control",
            );
        });
        cx.simulate_keystrokes("enter");
        frame.update(cx, |frame, _| {
            let PatchInspectionPhase::Ready { snapshot, .. } = frame.patches.phase() else {
                panic!("expected refreshed patch state");
            };
            assert_eq!(
                snapshot.patch_status(PatchID::DebugMenu),
                PatchStatus::Applied
            );
        });
    }

    fn test_window(cx: &mut TestAppContext, root: &Path) -> gpui::WindowHandle<AppFrame> {
        let startup = test_startup(root);
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        })
    }

    fn test_startup(root: &Path) -> SettingsStartup {
        let settings = root.join("settings.json");
        let mut startup = SettingsStartup::load(settings);
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(root.to_path_buf()));
        startup
    }

    fn draw_patch_frame(cx: &mut VisualTestContext, frame: &Entity<AppFrame>) {
        let frame = frame.clone();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(1180.0), px(780.0)),
            move |_, _| frame,
        );
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing click target {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
    }

    struct ExecutableFixture {
        directory: TempDir,
        path: PathBuf,
    }

    impl ExecutableFixture {
        fn original() -> Self {
            let directory = TempDir::new().unwrap();
            let path = directory.path().join("Kuf2Main.exe");
            File::create(&path).unwrap().set_len(0x002C_0CB8).unwrap();
            let fixture = Self { directory, path };
            fixture.write(0x000D_76EC, &[0x8B, 0x35, 0xB0, 0x3C, 0x74, 0x00]);
            fixture.write(0x000D_7710, &[0x8B, 0x0D, 0xB0, 0x3C, 0x74, 0x00]);
            fixture.write(0x0022_D991, &[0xE8, 0x8A, 0x95, 0x01, 0x00]);
            fixture.write(0x002B_951E, &[0; 87]);
            fixture.write(0x0007_1914, &[0xC7, 0x86, 0xD0, 0x0A, 0x00, 0x00]);
            fixture.write(0x0007_47CF, &[0x8B, 0x87, 0xDC, 0x0A, 0x00, 0x00]);
            fixture.write(0x0007_47D8, &[0x89, 0x87, 0xD4, 0x0A, 0x00, 0x00]);
            fixture.write(0x0007_191A, &5_i32.to_le_bytes());
            fixture.write(0x0007_47D5, &[0x8D, 0x04, 0x40]);
            fixture.write(0x002C_0CB4, &(-0.009_f32).to_bits().to_le_bytes());
            fixture
        }

        fn root(&self) -> &Path {
            self.directory.path()
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn backup_path(&self) -> PathBuf {
            self.directory.path().join("Kuf2Main.exe.bak")
        }

        fn write(&self, offset: u64, bytes: &[u8]) {
            let mut file = fs::OpenOptions::new().write(true).open(&self.path).unwrap();
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.write_all(bytes).unwrap();
        }
    }
}
