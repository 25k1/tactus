use super::media_list::MediaList;
use super::view_model::{Content, PageButton, Sort, Status, arrange};
use super::widgets::*;
use crate::i18n::fl;
use std::cell::RefCell;
use tactus_core::player::{MY_WAVE, wave};
use tactus_core::settings::Column;
use wxdragon::prelude::*;

fn wave_settings() -> [(String, Vec<(String, &'static str)>); 3] {
    fn options<const N: usize>(labels: [String; N], seeds: [&'static str; N]) -> Vec<(String, &'static str)> {
        std::iter::once((fl!("wave-any"), "")).chain(labels.into_iter().zip(seeds)).collect()
    }
    [
        (
            fl!("wave-character"),
            options([fl!("wave-favorite"), fl!("wave-discover"), fl!("wave-popular")], wave::CHARACTERS),
        ),
        (
            fl!("wave-mood"),
            options([fl!("wave-active"), fl!("wave-fun"), fl!("wave-calm"), fl!("wave-sad")], wave::MOODS),
        ),
        (
            fl!("wave-language"),
            options([fl!("wave-russian"), fl!("wave-foreign"), fl!("wave-instrumental")], wave::LANGUAGES),
        ),
    ]
}

pub struct Browser {
    pub panel: Panel,
    search_panel: Panel,
    pub search: ComboBox,
    pub search_button: Button,
    wave_panel: Panel,
    wave_choices: Vec<Choice>,
    pub wave_start: Button,
    heading: StaticText,
    description: TextCtrl,
    pub buttons: Vec<(PageButton, Button)>,
    section_label: StaticText,
    pub section: Choice,
    section_labels: RefCell<Vec<String>>,
    pub filter: TextCtrl,
    pub sort: Choice,
    list_label: StaticText,
    pub list: MediaList,
}

impl Browser {
    pub fn new(parent: &impl WxWidget, columns: &[Column]) -> Browser {
        let panel = Panel::builder(parent).build();
        let sizer = column();

        let search_panel = Panel::builder(&panel).build();
        let search_row = row();
        let search = field(&search_row, &search_panel, &fl!("search-label"), true, || {
            ComboBox::builder(&search_panel).with_style(ComboBoxStyle::Default | ComboBoxStyle::ProcessEnter).build()
        });
        let search_button = button(&search_panel, &fl!("search-button"));
        search_row.add(&search_button, 0, SizerFlag::AlignCenterVertical, 0);
        search_panel.set_sizer(search_row, true);

        let wave_panel = Panel::builder(&panel).build();
        let wave_row = row();
        let wave_choices: Vec<Choice> = wave_settings()
            .into_iter()
            .map(|(name, options)| {
                let labels: Vec<String> = options.into_iter().map(|(label, _)| label).collect();
                field(&wave_row, &wave_panel, &name, false, || choice(&wave_panel, &labels, 0))
            })
            .collect();
        let wave_start = button(&wave_panel, &fl!("wave-start"));
        wave_row.add(&wave_start, 0, SizerFlag::AlignCenterVertical | SizerFlag::All, 6);
        wave_panel.set_sizer(wave_row, true);

        let heading = label(&panel, "");
        let description = read_only_text(&panel, &fl!("page-description"), 80);
        let buttons_row = row();
        let buttons: Vec<(PageButton, Button)> = PageButton::ALL
            .into_iter()
            .map(|kind| {
                let control = button(&panel, &kind.label());
                buttons_row.add(&control, 0, centered(), 6);
                (kind, control)
            })
            .collect();
        let section_label = label(&panel, &fl!("page-show"));
        let section = choice(&panel, &[], 0);
        buttons_row.add_spacer(12);
        buttons_row.add(&section_label, 0, centered(), 4);
        buttons_row.add(&section, 0, SizerFlag::AlignCenterVertical, 0);
        let arrange_row = row();
        let filter = field(&arrange_row, &panel, &fl!("page-filter"), true, || TextCtrl::builder(&panel).build());
        let sorts: Vec<String> = Sort::ALL.into_iter().map(Sort::label).collect();
        let sort = field(&arrange_row, &panel, &fl!("page-sort"), false, || choice(&panel, &sorts, 0));
        let list_label = label(&panel, &fl!("page-list"));
        let list = MediaList::new(&panel, &fl!("page-list"), columns);

        sizer.add(&search_panel, 0, SizerFlag::Expand | SizerFlag::All, 6);
        sizer.add(&wave_panel, 0, SizerFlag::Expand | SizerFlag::All, 6);
        sizer.add(&heading, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
        sizer.add(&description, 0, SizerFlag::Expand | SizerFlag::All, 6);
        sizer.add_sizer(&buttons_row, 0, SizerFlag::Expand | SizerFlag::All, 6);
        sizer.add_sizer(&arrange_row, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 2);
        sizer.add(&list_label, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
        sizer.add(&list.list, 1, SizerFlag::Expand | SizerFlag::All, 6);
        panel.set_sizer(sizer, true);

        Browser {
            panel,
            search_panel,
            search,
            search_button,
            wave_panel,
            wave_choices,
            wave_start,
            heading,
            description,
            buttons,
            section_label,
            section,
            section_labels: RefCell::new(Vec::new()),
            filter,
            sort,
            list_label,
            list,
        }
    }

    pub fn wave_seeds(&self) -> Vec<String> {
        let mut seeds = vec![MY_WAVE.to_owned()];
        for (control, (_, options)) in self.wave_choices.iter().zip(wave_settings()) {
            if let Some((_, seed)) = options.get(selection(control)).filter(|(_, seed)| !seed.is_empty()) {
                seeds.push((*seed).to_owned());
            }
        }
        seeds
    }

    pub fn sort_order(&self) -> Sort {
        Sort::ALL.get(selection(&self.sort)).copied().unwrap_or_default()
    }

    pub fn arranged(&self) -> bool {
        !self.filter.get_value().trim().is_empty() || self.sort_order() != Sort::Original
    }

    pub fn arrangement(&self) -> (String, usize) {
        (self.filter.get_value(), selection(&self.sort))
    }

    pub fn set_arrangement(&self, filter: &str, sort: usize) {
        self.filter.change_value(filter);
        self.sort.set_selection(sort.min(Sort::ALL.len() - 1) as u32);
    }

    pub fn section_index(&self) -> usize {
        selection(&self.section)
    }

    pub fn heading(content: &Content, searching: bool) -> String {
        match &content.status {
            Status::Ready => content.title.clone(),
            Status::Loading => fl!("page-loading", title = content.title.clone()),
            Status::Failed(error) => fl!("page-failed", title = content.title.clone(), error = error.clone()),
            Status::Empty if searching => fl!("search-hint"),
            Status::Empty => fl!("page-empty", title = content.title.clone()),
        }
    }

    pub fn render(
        &self,
        content: &Content,
        buttons: &[PageButton],
        section: usize,
        row: Option<usize>,
        marked: Option<usize>,
        mode: (bool, bool),
    ) {
        let (searching, wave) = mode;
        self.search_panel.show(searching);
        self.wave_panel.show(wave);
        let heading = Self::heading(content, searching);
        self.heading.set_label(&heading);
        self.description.change_value(&content.description);
        self.description.show(!content.description.is_empty());
        for (kind, control) in &self.buttons {
            control.show(buttons.contains(kind));
        }
        let labels: Vec<String> = content.sections.iter().map(|section| section.label.clone()).collect();
        let section = section.min(labels.len().saturating_sub(1));
        if *self.section_labels.borrow() != labels {
            set_choices(&self.section, &labels, section);
            *self.section_labels.borrow_mut() = labels.clone();
        } else if self.section_index() != section && !labels.is_empty() {
            self.section.set_selection(section as u32);
        }
        let multiple = labels.len() > 1;
        self.section_label.show(multiple);
        self.section.show(multiple);
        let entries = content.sections.get(section).map(|section| section.entries.as_slice()).unwrap_or_default();
        let rows = arrange(entries, &self.filter.get_value(), self.sort_order());
        let name = match content.sections.get(section) {
            Some(current) if multiple => format!("{}: {}", content.title, current.label),
            _ => heading.clone(),
        };
        let list_name = if self.filter.get_value().trim().is_empty() {
            name
        } else {
            fl!("list-filtered", name = name, shown = rows.len(), total = entries.len())
        };
        self.list_label.set_label(&list_name);
        self.list.list.set_accessibility_label(&list_name);
        self.list.set(rows, marked, row);
        self.panel.layout();
    }

    pub fn set_suggestions(&self, texts: &[String]) {
        let value = self.search.get_value();
        let point = self.search.get_insertion_point();
        self.search.clear();
        for text in texts {
            self.search.append(text);
        }
        self.search.change_value(&value);
        self.search.set_insertion_point(point);
    }
}
