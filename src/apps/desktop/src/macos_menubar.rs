//! macOS Native Menubar

#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, SubmenuBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenubarMode {
    Startup,
    Workspace,
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct MenubarLabels {
    project_menu: &'static str,
    navigation_menu: &'static str,
    edit_menu: &'static str,
    open_project: &'static str,
    new_project: &'static str,
    go_home: &'static str,
    about_bitfun: &'static str,
}

#[cfg(target_os = "macos")]
fn labels_for_language(language: &str) -> MenubarLabels {
    match language {
        "en-US" => MenubarLabels {
            project_menu: "Project",
            navigation_menu: "Navigation",
            edit_menu: "Edit",
            open_project: "Open Project…",
            new_project: "New Project…",
            go_home: "Go Home",
            about_bitfun: "About BitFun",
        },
        _ => MenubarLabels {
            project_menu: "工程",
            navigation_menu: "导航",
            edit_menu: "编辑",
            open_project: "打开工程…",
            new_project: "新建工程…",
            go_home: "返回首页",
            about_bitfun: "关于 BitFun",
        },
    }
}

#[cfg(target_os = "macos")]
pub fn set_macos_menubar_with_mode(
    app: &tauri::AppHandle,
    language: &str,
    mode: MenubarMode,
) -> tauri::Result<()> {
    let labels = labels_for_language(language);

    let menu = match mode {
        MenubarMode::Startup => {
            let app_menu = SubmenuBuilder::new(app, "BitFun")
                .text("bitfun.about", labels.about_bitfun)
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, labels.edit_menu)
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            MenuBuilder::new(app)
                .item(&app_menu)
                .item(&edit_menu)
                .build()?
        }
        MenubarMode::Workspace => {
            let app_menu = SubmenuBuilder::new(app, "BitFun")
                .text("bitfun.about", labels.about_bitfun)
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, labels.edit_menu)
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let project_menu = SubmenuBuilder::new(app, labels.project_menu)
                .text("bitfun.open_project", labels.open_project)
                .text("bitfun.new_project", labels.new_project)
                .build()?;

            let navigation_menu = SubmenuBuilder::new(app, labels.navigation_menu)
                .text("bitfun.go_home", labels.go_home)
                .build()?;

            MenuBuilder::new(app)
                .item(&app_menu)
                .item(&edit_menu)
                .item(&project_menu)
                .item(&navigation_menu)
                .build()?
        }
    };

    app.set_menu(menu)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_macos_menubar_with_mode(
    _app: &tauri::AppHandle,
    _language: &str,
    _mode: MenubarMode,
) -> tauri::Result<()> {
    Ok(())
}
