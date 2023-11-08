use anyhow::{Context, Error};
use arboard::Clipboard;
use chrono::{DateTime, Utc};
use rfd::{
    AsyncFileDialog, FileHandle, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel,
};
use ruffle_core::backend::navigator::OpenURLMode;
use ruffle_core::backend::ui::{
    DialogLoaderError, DialogResultFuture, FileDialogResult, FileFilter, FontDefinition,
    FullscreenError, LanguageIdentifier, MouseCursor, UiBackend, US_ENGLISH,
};
use std::rc::Rc;
use sys_locale::get_locale;
use tracing::error;
use url::Url;
use winit::window::{Fullscreen, Window};

pub struct DesktopFileDialogResult {
    handle: Option<FileHandle>,
    md: Option<std::fs::Metadata>,
    contents: Vec<u8>,
}

impl DesktopFileDialogResult {
    /// Create a new [`DesktopFileDialogResult`] from a given file handle
    pub fn new(handle: Option<FileHandle>) -> Self {
        let md = handle
            .as_ref()
            .and_then(|x| std::fs::metadata(x.path()).ok());

        let contents = handle
            .as_ref()
            .and_then(|handle| std::fs::read(handle.path()).ok())
            .unwrap_or_default();

        Self {
            handle,
            md,
            contents,
        }
    }
}

impl FileDialogResult for DesktopFileDialogResult {
    fn is_cancelled(&self) -> bool {
        self.handle.is_none()
    }

    fn creation_time(&self) -> Option<DateTime<Utc>> {
        if let Some(md) = &self.md {
            md.created().ok().map(DateTime::<Utc>::from)
        } else {
            None
        }
    }

    fn modification_time(&self) -> Option<DateTime<Utc>> {
        if let Some(md) = &self.md {
            md.modified().ok().map(DateTime::<Utc>::from)
        } else {
            None
        }
    }

    fn file_name(&self) -> Option<String> {
        self.handle.as_ref().map(|handle| handle.file_name())
    }

    fn size(&self) -> Option<u64> {
        self.md.as_ref().map(|md| md.len())
    }

    fn file_type(&self) -> Option<String> {
        if let Some(handle) = &self.handle {
            handle
                .path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| ".".to_owned() + x)
        } else {
            None
        }
    }

    fn creator(&self) -> Option<String> {
        None
    }

    fn contents(&self) -> &[u8] {
        &self.contents
    }

    fn write(&self, data: &[u8]) {
        if let Some(handle) = &self.handle {
            let _ = std::fs::write(handle.path(), data);
        }
    }

    fn refresh(&mut self) {
        let md = self
            .handle
            .as_ref()
            .and_then(|x| std::fs::metadata(x.path()).ok());

        let contents = if let Some(handle) = &self.handle {
            std::fs::read(handle.path()).unwrap_or_default()
        } else {
            Vec::new()
        };

        self.md = md;
        self.contents = contents;
    }
}

pub struct DesktopUiBackend {
    window: Rc<Window>,
    cursor_visible: bool,
    clipboard: Clipboard,
    language: LanguageIdentifier,
    preferred_cursor: MouseCursor,
    open_url_mode: OpenURLMode,
    /// Is a dialog currently open
    dialog_open: bool,
}

impl DesktopUiBackend {
    pub fn new(window: Rc<Window>, open_url_mode: OpenURLMode) -> Result<Self, Error> {
        let preferred_language = get_locale();
        let language = preferred_language
            .and_then(|l| l.parse().ok())
            .unwrap_or_else(|| US_ENGLISH.clone());
        Ok(Self {
            window,
            cursor_visible: true,
            clipboard: Clipboard::new().context("Couldn't get platform clipboard")?,
            language,
            preferred_cursor: MouseCursor::Arrow,
            open_url_mode,
            dialog_open: false,
        })
    }

    pub fn cursor(&self) -> egui::CursorIcon {
        if self.cursor_visible {
            match self.preferred_cursor {
                MouseCursor::Arrow => egui::CursorIcon::Default,
                MouseCursor::Hand => egui::CursorIcon::PointingHand,
                MouseCursor::IBeam => egui::CursorIcon::Text,
                MouseCursor::Grab => egui::CursorIcon::Grab,
            }
        } else {
            egui::CursorIcon::None
        }
    }
}

const DOWNLOAD_FAILED_MESSAGE: &str = "Ruffle failed to open or download this file.";

impl UiBackend for DesktopUiBackend {
    fn mouse_visible(&self) -> bool {
        self.cursor_visible
    }

    fn set_mouse_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        self.preferred_cursor = cursor;
    }

    fn clipboard_content(&mut self) -> String {
        self.clipboard.get_text().unwrap_or_default()
    }

    fn set_clipboard_content(&mut self, content: String) {
        if let Err(e) = self.clipboard.set_text(content) {
            error!("Couldn't set clipboard contents: {:?}", e);
        }
    }

    fn set_fullscreen(&mut self, is_full: bool) -> Result<(), FullscreenError> {
        self.window.set_fullscreen(if is_full {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
        Ok(())
    }

    fn display_root_movie_download_failed_message(&self) {
        let dialog = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title("Ruffle - Load failed")
            .set_description(DOWNLOAD_FAILED_MESSAGE)
            .set_buttons(MessageButtons::Ok);
        dialog.show();
    }

    fn message(&self, message: &str) {
        let dialog = MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title("Ruffle")
            .set_description(message)
            .set_buttons(MessageButtons::Ok);
        dialog.show();
    }

    fn display_unsupported_video(&self, url: Url) {
        if url.scheme() == "javascript" {
            tracing::warn!(
                "SWF tried to run a script on desktop, but javascript calls are not allowed"
            );
            return;
        }

        if self.open_url_mode == OpenURLMode::Confirm {
            let message = format!("The SWF file wants to open the website {}", url);
            // TODO: Add a checkbox with a GUI toolkit
            let confirm = MessageDialog::new()
                .set_title("Open website?")
                .set_level(MessageLevel::Info)
                .set_description(message)
                .set_buttons(MessageButtons::OkCancel)
                .show()
                == MessageDialogResult::Ok;
            if !confirm {
                tracing::info!("SWF tried to open a website, but the user declined the request");
                return;
            }
        } else if self.open_url_mode == OpenURLMode::Deny {
            tracing::warn!("SWF tried to open a website, but opening a website is not allowed");
            return;
        }

        // If the user confirmed or if in Allow mode, open the website

        // TODO: This opens local files in the browser while flash opens them
        // in the default program for the respective filetype.
        // This especially includes mailto links. Ruffle opens the browser which opens
        // the preferred program while flash opens the preferred program directly.

        match webbrowser::open(url.as_str()) {
            Ok(_output) => {}
            Err(e) => tracing::error!("Could not open URL {}: {}", url, e),
        };
    }

    fn load_device_font(&self, _name: &str, _register: &dyn FnMut(FontDefinition)) {}

    // Unused on desktop
    fn open_virtual_keyboard(&self) {}

    fn language(&self) -> &LanguageIdentifier {
        &self.language
    }

    fn display_file_open_dialog(&mut self, filters: Vec<FileFilter>) -> Option<DialogResultFuture> {
        // Prevent opening multiple dialogs at the same time
        if self.dialog_open {
            return None;
        }
        self.dialog_open = true;

        // Create the dialog future
        Some(Box::pin(async move {
            let mut dialog = AsyncFileDialog::new();

            for filter in filters {
                if cfg!(target_os = "macos") && filter.mac_type.is_some() {
                    let mac_type = filter.mac_type.expect("Checked above");
                    let extensions: Vec<&str> = mac_type.split(';').collect();
                    dialog = dialog.add_filter(&filter.description, &extensions);
                } else {
                    let extensions: Vec<&str> = filter
                        .extensions
                        .split(';')
                        .map(|x| x.trim_start_matches("*."))
                        .collect();
                    dialog = dialog.add_filter(&filter.description, &extensions);
                }
            }

            let result: Result<Box<dyn FileDialogResult>, DialogLoaderError> = Ok(Box::new(
                DesktopFileDialogResult::new(dialog.pick_file().await),
            ));
            result
        }))
    }

    fn display_file_save_dialog(
        &mut self,
        file_name: String,
        title: String,
    ) -> Option<DialogResultFuture> {
        // Prevent opening multiple dialogs at the same time
        if self.dialog_open {
            return None;
        }
        self.dialog_open = true;

        // Create the dialog future
        Some(Box::pin(async move {
            // Select the location to save the file to
            let dialog = AsyncFileDialog::new()
                .set_title(&title)
                .set_file_name(&file_name);

            let result: Result<Box<dyn FileDialogResult>, DialogLoaderError> = Ok(Box::new(
                DesktopFileDialogResult::new(dialog.save_file().await),
            ));
            result
        }))
    }

    fn close_file_dialog(&mut self) {
        self.dialog_open = false;
    }
}
