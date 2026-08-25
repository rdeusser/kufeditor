use gpui::Context;
use kufeditor_game::Game;

use super::AppFrame;
use crate::{
    notices::{Notice, NoticeLevel, NoticeSource},
    settings::{
        SettingsImageError, SettingsImageV1, SettingsQueueResult, SettingsRevision,
        SettingsWriteCompletion, image_from_runtime,
    },
};

pub(super) fn protected_settings_notice(found: u64) -> Notice {
    Notice::plain(
        NoticeLevel::Warning,
        format!("Settings version {found} is unsupported; changes will not be saved"),
    )
}

fn image_error_notice(error: &SettingsImageError) -> Notice {
    Notice::error("Could not prepare application settings", error)
}

fn begin_write_notice(frame: &mut AppFrame, revision: SettingsRevision, notice: Notice) {
    frame
        .notices
        .begin(NoticeSource::SettingsWrite, revision.get(), notice);
}

impl AppFrame {
    pub(super) fn schedule_settings_write(&mut self, game: Game, cx: &mut Context<Self>) -> bool {
        let image: SettingsImageV1 =
            match image_from_runtime(game, &self.game_paths, &self.recent_files) {
                Ok(image) => image,
                Err(error) => {
                    self.settings.discard_obsolete();
                    self.close_pending = false;
                    self.close_armed = false;
                    self.notices
                        .replace(NoticeSource::SettingsWrite, image_error_notice(&error));
                    cx.notify();
                    return false;
                }
            };

        match self.settings.queue(image) {
            SettingsQueueResult::Queued(revision) => {
                begin_write_notice(self, revision, Notice::info("Saving application settings"));
                self.start_next_settings_write(cx);
            }
            SettingsQueueResult::Protected(revision) => {
                let found = match self.settings.mode() {
                    crate::settings::PersistenceMode::Enabled => return false,
                    crate::settings::PersistenceMode::ProtectedUnsupportedVersion { found } => {
                        *found
                    }
                };
                begin_write_notice(self, revision, protected_settings_notice(found));
            }
        }
        cx.notify();
        true
    }

    pub(super) fn start_next_settings_write(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.settings.take_ready() else {
            return;
        };
        let task = cx.background_executor().spawn(async move { request.run() });
        cx.spawn(async move |entity, cx| {
            let completion = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_settings_write(completion, cx);
            });
        })
        .detach();
    }

    pub(super) fn finish_settings_write(
        &mut self,
        completion: SettingsWriteCompletion,
        cx: &mut Context<Self>,
    ) {
        let finish = self.settings.finish(completion);
        if finish.is_latest {
            match finish.result {
                Ok(()) => {
                    self.notices
                        .complete(NoticeSource::SettingsWrite, finish.revision.get(), None);
                    self.notices.clear(NoticeSource::Startup);
                }
                Err(error) => {
                    self.close_pending = false;
                    self.close_armed = false;
                    self.notices.complete(
                        NoticeSource::SettingsWrite,
                        finish.revision.get(),
                        Some(Notice::error("Could not save application settings", &error)),
                    );
                }
            }
        }
        self.start_next_settings_write(cx);
        self.continue_close(cx);
        cx.notify();
    }
}
