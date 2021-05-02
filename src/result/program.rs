use std::fs;
use std::ffi::OsStr;
use std::path::PathBuf;

use gtk::prelude::*;
use convert_case::{ Case, Casing };
use freedesktop_entry_parser::parse_entry;

use super::SearchResult;


/**
 * Unwraps an Ok result, or calls continue.
 */

macro_rules! or_continue {
	($res: expr) => {
		match $res {
			Ok(val) => val,
			Err(_) => continue
		}
	};
}


/**
 * Categories to be ignored when discovering the displayed category of a program.
 * These categories are either too general, for development purposes, or don't
 * present useful information to regular users.
 */

const EXCLUDED_CATEGORIES: [&str; 10] = [
	"APPLICATION",
	"CONSOLEONLY",
	"NETWORK",
	"FILETRANSFER",
	"TEXTEDITOR",
	"X-XFCE",
	"GNOME",
	"XFCE",
	"GTK",
	"KDE"
];


/**
 * Represents a desktop action.
 */

#[derive(Debug, Clone)]
pub struct Action {
	pub name: String,
	pub exec: String
}

/**
 * A program search result, created from a desktop entry.
 * Activates a program using a shell command when activated.
 */

#[derive(Debug, Clone)]
pub struct ProgramResult {
	name: String,
	category: String,
	description: String,
	icon: Option<String>,
	version: Option<String>,

	exec: String,
	actions: Option<Vec<Action>>,

	widget: gtk::Box,
	top_button: gtk::Button
}

impl ProgramResult {

	/**
	 * Finds all desktop programs on the system, and creates search results for them.
	 */

	pub fn find_all() -> Vec<Self> {
		let mut search_paths: Vec<PathBuf> = vec![
			"/home/auri/.local/share/applications".into(),
			"/usr/share/applications".into(),
			"/usr/local/share/applications".into()
		];

		let mut found = Vec::<ProgramResult>::new();

		while search_paths.len() != 0 {
			let path = search_paths.pop().unwrap();
			let dir_iter = or_continue!(fs::read_dir(&path));
			
			for entry in dir_iter {
				let entry = or_continue!(entry);
				let path = entry.path();
				
				if path.is_dir() {
					search_paths.push(path);
					continue;
				}

				if path.extension() == Some(OsStr::new("desktop")) {
					let parsed = or_continue!(parse_entry(path));
					let entry = parsed.section("Desktop Entry");

					let show = entry.attr("NoDisplay").unwrap_or("false") == "false" && entry.attr("Hidden").unwrap_or("false") == "false";

					let action_names = entry.attr("Actions").and_then(|s| Some(s.split(';')
						.filter(|s| !s.is_empty()).collect())).unwrap_or_else(|| vec![]);
					let actions = if action_names.len() > 0 {
						Some(action_names.iter().map(|name| {
							let entry = parsed.section(["Desktop Action", name].join(" "));
							Action {
								name: entry.attr("Name").unwrap_or("Unnamed Action").to_owned(),
								exec: entry.attr("Exec").unwrap().to_owned(),
							}
						}).collect())
					} else { None };
					
					let exec = entry.attr("Exec");
					if exec.is_none() { continue; }

					if show {
						found.push(ProgramResult::new(
							entry.attr("Name").unwrap_or("Unnamed Application"),
							entry.attr("Comment").unwrap_or(""),
							&ProgramResult::choose_category(entry.attr("Categories")),
							entry.attr("Version"),
							exec.unwrap(),
							entry.attr("Icon"),
							actions
						))
					}
				}
			}
		}

		found
	}


	/**
	 * Removes template parameters from a shell command.
	 */

	pub fn format_exec(exec: &str) -> String {
		exec.replace("%f", "").replace("%F", "").replace("%D", "~").replace("%u", "").replace("%U", "")
	}


	/**
	 * Chooses the best category to display in the result.
	 */

	pub fn choose_category(list: Option<&str>) -> String {
		let list: Vec<_> = list.unwrap_or("").split(";")
			.filter(|s| !EXCLUDED_CATEGORIES.contains(&s.to_uppercase().as_str())).collect();
		list.get(0).map_or("Application", |s| &s).to_case(Case::Title).to_uppercase()
	}


	/**
	 * Creates a new Program result, with a corresponding result widget.
	 */

	pub fn new(name: &str, description: &str, category: &str, version: Option<&str>,
		exec: &str, icon: Option<&str>, actions: Option<Vec<Action>>) -> Self {
		
		let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
		widget.get_style_context().add_class("result");
		let top_button = gtk::Button::new();

		{
			top_button.get_style_context().add_class("flat");
			widget.pack_start(&top_button, true, true, 0);

			let exec = ProgramResult::format_exec(exec);
			top_button.connect_clicked(move |_| {
				println!("Executing '{}'", &exec);
				let args = shell_words::split(&exec).unwrap();
				std::process::Command::new(&args[0]).args(&args[1..])
					.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
			});

			let widget_top = gtk::Box::new(gtk::Orientation::Horizontal, 4);
			top_button.add(&widget_top);

			let icon_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
			icon_box.get_style_context().add_class("result_application_icon_box");
			widget_top.pack_start(&icon_box, false, false, 4);

			let icon = gtk::Image::from_icon_name(icon, gtk::IconSize::Dnd);
			icon.set_size_request(32, 32);
			icon_box.pack_start(&icon, false, false, 0);

			let description_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
			widget_top.pack_start(&description_box, true, true, 0);

			let category_label = gtk::Label::new(Some(&["<span size='small' weight='bold'>", category, "</span>"].join("")));
			category_label.get_style_context().add_class("category_label");
			category_label.set_ellipsize(pango::EllipsizeMode::End);
			category_label.set_use_markup(true);
			category_label.set_xalign(0.0);
			description_box.pack_start(&category_label, false, false, 1);

			let label = gtk::Label::new(Some(name));
			label.set_ellipsize(pango::EllipsizeMode::End);
			label.set_xalign(0.0);
			description_box.pack_start(&label, false, false, 1);

			if let Some(actions) = actions.as_ref() {
				let widget_actions = gtk::Box::new(gtk::Orientation::Vertical, 0);
				widget.pack_start(&widget_actions, true, true, 0);

				for action in actions {
					let widget_action_button = gtk::Button::new();
					widget_action_button.get_style_context().add_class("flat");
					widget_action_button.get_style_context().add_class("action_button");
					widget_actions.pack_start(&widget_action_button, true, true, 0);

					let exec = ProgramResult::format_exec(&action.exec);
					widget_action_button.connect_clicked(move |_| {
						println!("Executing '{}'", &exec);
						let args = shell_words::split(&exec).unwrap();
						std::process::Command::new(&args[0]).args(&args[1..])
							.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
					});

					let widget_action = gtk::Box::new(gtk::Orientation::Horizontal, 0);
					widget_action_button.add(&widget_action);

					let icon_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
					icon_box.get_style_context().add_class("action_icon_box");
					widget_action.pack_start(&icon_box, false, false, 4);

					let icon = gtk::Image::from_icon_name(Some("start-here-symbolic"), gtk::IconSize::Button);
					icon.set_size_request(16, 16);
					icon_box.pack_start(&icon, false, false, 0);

					let action_label = gtk::Label::new(Some(&action.name));
					action_label.set_ellipsize(pango::EllipsizeMode::End);
					action_label.set_xalign(0.0);
					widget_action.pack_start(&action_label, false, false, 4);
				}
			}
		}

		ProgramResult {
			name: name.to_owned(),
			category: category.to_owned(),
			description: description.to_owned(),
			icon: icon.and_then(|s| Some(s.to_owned())),
			version: version.and_then(|s| Some(s.to_owned())),
			exec: exec.to_owned(),
			top_button,
			actions,
			widget
		}
	}
}

impl SearchResult for ProgramResult {
	fn get_ranking(&self, query: &str) -> usize {
		let mut score = 0;
		let mut last_letter_ind: usize = 0;
		let mut lowercase_name = self.name.to_lowercase();
		lowercase_name.retain(|c| !c.is_whitespace());

		for letter in query.chars() {
			let mut name_chars = lowercase_name.chars().skip(last_letter_ind);
			let pos = name_chars.position(|c| c == letter).map_or(-1, |c| c as isize);
			if pos >= 0 {
				last_letter_ind += pos as usize + 1;
				score += std::cmp::max(10 - pos, 0) as usize
			}
		}

		score
	}

	fn set_first(&self, first: bool) -> () {
		self.top_button.set_can_focus(!first);
	}

	fn activate(&self) {
		let exec = ProgramResult::format_exec(&self.exec);
		println!("Executing '{}'", &exec);
		let args = shell_words::split(&exec).unwrap();
		std::process::Command::new(&args[0]).args(&args[1..])
			.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
	}

	fn get_result_widget(&self) -> gtk::Widget {
		self.widget.clone().upcast()
	}

	fn get_preview_widget(&self) -> gtk::Widget {
		let widget = gtk::Box::new(gtk::Orientation::Vertical, 4);
		widget.set_border_width(24);

		let icon_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
		icon_box.get_style_context().add_class("preview_application_icon_box");
		icon_box.set_size_request(64, 64);
		icon_box.set_halign(gtk::Align::Center);
		widget.pack_start(&icon_box, false, false, 24);

		let icon = gtk::Image::from_icon_name(self.icon.as_ref().and_then(|s| Some(s.as_str())), gtk::IconSize::Dialog);
		icon_box.pack_start(&icon, false, false, 0);

		let category_label = gtk::Label::new(Some(&["<span size='small' weight='bold'>", &self.category, "</span>"].join("")));
		category_label.get_style_context().add_class("category_label");
		category_label.set_ellipsize(pango::EllipsizeMode::End);
		category_label.set_use_markup(true);
		widget.pack_start(&category_label, false, false, 2);

		let label = gtk::Label::new(Some(&self.name));
		label.set_ellipsize(pango::EllipsizeMode::End);
		widget.pack_start(&label, false, false, 2);

		let description = gtk::Label::new(Some(&[ &self.description, "." ].join("")));
		description.get_style_context().add_class("secondary");

		description.set_line_wrap_mode(pango::WrapMode::WordChar);
		description.set_ellipsize(pango::EllipsizeMode::End);
		description.set_justify(gtk::Justification::Center);
		description.set_halign(gtk::Align::Center);
		description.set_max_width_chars(36);
		description.set_line_wrap(true);
		description.set_lines(5);

		widget.pack_start(&description, false, false, 4);

		if let Some(version) = self.version.as_ref() {
			let version_label = gtk::Label::new(Some(&["<span size='small'>VERSION ", &version, "</span>"].join("")));
			version_label.get_style_context().add_class("category_label");
			version_label.set_ellipsize(pango::EllipsizeMode::End);
			version_label.set_use_markup(true);
			version_label.set_valign(gtk::Align::End);
			widget.pack_start(&version_label, true, true, 2);
		}

		let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
		button_box.get_style_context().add_class("preview_application_button_box");
		button_box.get_style_context().add_class("linked");
		button_box.set_halign(gtk::Align::Center);
		button_box.set_valign(gtk::Align::End);
		widget.pack_end(&button_box, false, false, 4);

		let launch_button = gtk::Button::from_icon_name(Some("media-playback-start-symbolic"), gtk::IconSize::LargeToolbar);
		button_box.pack_start(&launch_button, false, false, 0);
		let favorite_button = gtk::Button::from_icon_name(Some("emblem-favorite-symbolic"), gtk::IconSize::LargeToolbar);
		button_box.pack_start(&favorite_button, false, false, 0);
		let edit_button = gtk::Button::from_icon_name(Some("document-edit-symbolic"), gtk::IconSize::LargeToolbar);
		button_box.pack_start(&edit_button, false, false, 0);

		return widget.upcast();
	}
}