#![allow(clippy::single_match)]
use ::std::collections::HashMap;
use ::std::sync::Mutex;
use anyhow::Error;
use chrono::naive::NaiveDate as Date;
use derive_more::Display;
use log::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
mod citation;
mod filters;
mod github;
mod md;

lazy_static::lazy_static! {
	static ref FOOTNOTES: Mutex<Option<HashMap<String, usize>>> = Mutex::new(Some(HashMap::new()));
}

struct DateRange {
	start: Date,
	end: Option<Date>,
}

impl DateRange {
	fn to_resume_string(&self) -> String {
		if let Some(end) = self.end {
			format!(
				"{} - {}",
				self.start.format("%b,&nbsp;%Y"),
				end.format("%b,&nbsp;%Y")
			)
		} else {
			format!("{} - Current", self.start.format("%b,&nbsp;%Y"))
		}
	}
}

impl std::fmt::Display for DateRange {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(end) = self.end {
			write!(f, "{}~{}", self.start.format("%Y-%m"), end.format("%Y-%m"))
		} else {
			write!(f, "{}~", self.start.format("%Y-%m"))
		}
	}
}

impl std::str::FromStr for DateRange {
	type Err = anyhow::Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let p: Vec<_> = s.split('~').collect();
		if p.len() != 2 {
			Err(anyhow::anyhow!(
				"A date range should have 2 and only 2 dates"
			))
		} else {
			Ok(DateRange {
				start: Date::parse_from_str(&format!("{}-01", p[0]), "%Y-%m-%d").unwrap(),
				end: if p[1].is_empty() {
					None
				} else {
					Some(Date::parse_from_str(&format!("{}-01", p[1]), "%Y-%m-%d").unwrap())
				},
			})
		}
	}
}

impl Serialize for DateRange {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		s.serialize_str(&self.to_string())
	}
}

impl<'a> Deserialize<'a> for DateRange {
	fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Self, D::Error> {
		let s = String::deserialize(d)?;
		s.parse().map_err(serde::de::Error::custom)
	}
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Citation {
	Raw(String),
	RawWithYear { text: String, year: Option<u32> },
	Url(citation::UrlCitation),
	Doi(citation::DoiCitation),
	Bibtex(citation::BibtexCitation),
}

impl Citation {
	fn to_raw(&self) -> Option<Citation> {
		use Citation::*;
		match self {
			Raw(s) => Some(Raw(s.clone())),
			RawWithYear { text, .. } => Some(Raw(text.clone())),
			Url(url) => url.to_raw(),
			Doi(doi) => doi.to_raw(),
			Bibtex(bib) => bib.to_raw(),
		}
	}
	fn set_year(self, year: Option<u32>) -> Citation {
		use Citation::*;
		if let Raw(s) = self {
			RawWithYear { text: s, year }
		} else {
			self
		}
	}
	fn to_raw_with_year(&self) -> Option<Citation> {
		use Citation::*;
		match self {
			Raw(s) => Some(RawWithYear {
				text: s.clone(),
				year: None,
			}),
			RawWithYear { text, year } => Some(RawWithYear {
				text: text.clone(),
				year: *year,
			}),
			Url(url) => url.to_raw().map(|v| v.set_year(url.year())),
			Doi(doi) => doi.to_raw().map(|v| v.set_year(doi.year())),
			Bibtex(bib) => bib.to_raw().map(|v| v.set_year(bib.year())),
		}
	}
}

#[derive(Serialize, Deserialize)]
enum Degree {
	BS,
	MS,
	PhD,
}

impl Degree {
	fn to_resume_string(&self) -> String {
		match self {
			Self::BS => "Bachelor of Science".into(),
			Self::MS => "Master of Science".into(),
			Self::PhD => "PhD".into(),
		}
	}
}

#[derive(Serialize, Deserialize)]
struct Education {
	institution: String,
	degree: Degree,
	major: String,
	duration: DateRange,
	#[serde(skip_serializing_if = "Option::is_none", default)]
	location: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none", default)]
	gpa: Option<f32>,
	#[serde(skip_serializing_if = "Option::is_none", default)]
	courses: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct Experience {
	company: String,
	position: String,
	duration: DateRange,
	description: String,
	#[serde(skip_serializing_if = "Option::is_none", default)]
	location: Option<String>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	tags: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Contact {
	#[serde(rename = "type")]
	type_: String,
	value: String,
}

#[derive(Serialize, Deserialize)]
struct Skill {
	category: String,
	#[serde(default)]
	description: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Person {
	name: String,
	#[serde(default)]
	resume_url: Option<String>,
	contacts: Vec<Contact>,
	educations: Vec<Education>,
	experiences: Vec<Experience>,
	projects: Vec<ProjectParam>,
	#[serde(default)]
	skills: Vec<Skill>,
	#[serde(default)]
	references: HashMap<String, Citation>,
	#[serde(default)]
	publications: Vec<Citation>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ProjectParam {
	Import(ProjectImport),
	Sort { order_by: ProjectSortOrder },
	ImportMode { import_mode: ProjectImportMode },
	Raw(Project),
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ProjectImportMode {
	Whitelist,
	Combine,
}

impl Default for ProjectImportMode {
	fn default() -> Self {
		Self::Combine
	}
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
enum ProjectSortOrder {
	Stars,
	Forks,
	StarsThenForks,
	ForksThenStars,
	Manual,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "lowercase")]
enum ProjectImport {
	GitHub {
		#[serde(default)]
		ignore_forks: bool,
		#[serde(default)]
		repos: Option<Vec<String>>,
		#[serde(default)]
		token: Option<String>,
	},
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Display)]
#[serde(rename_all = "lowercase")]
enum ProjectRole {
	Owner,
	Maintainer,
	Contributor,
}

/// Single digit precision deciaml real number
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
struct Decimal1(u64);
impl ::std::fmt::Display for Decimal1 {
	fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
		<f64 as ::std::fmt::Display>::fmt(&(*self).into(), f)
	}
}

impl ::std::ops::Add<Decimal1> for Decimal1 {
	type Output = Self;
	fn add(self, rhs: Self) -> Self::Output {
		Self(rhs.0 + self.0)
	}
}

impl ::std::ops::AddAssign<Decimal1> for Decimal1 {
	fn add_assign(&mut self, rhs: Self) {
		self.0 += rhs.0;
	}
}

impl From<f64> for Decimal1 {
	fn from(f: f64) -> Self {
		Self((f * 10.0) as u64)
	}
}

impl From<Decimal1> for f64 {
	fn from(f: Decimal1) -> f64 {
		f.0 as f64 / 10.0
	}
}

impl<'de> ::serde::Deserialize<'de> for Decimal1 {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct Visitor;
		impl<'de> ::serde::de::Visitor<'de> for Visitor {
			type Value = Decimal1;
			fn expecting(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				write!(fmt, "a float")
			}
			fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
			where
				E: ::serde::de::Error,
			{
				Ok(v.into())
			}
		}

		deserializer.deserialize_f64(Visitor)
	}
}

impl ::serde::Serialize for Decimal1 {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_f64((*self).into())
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LanguageStat {
	language: String,
	percentage: Decimal1,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Project {
	name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	contributions: Option<String>,
	#[serde(
		with = "serde_option_display_fromstr",
		default,
		skip_serializing_if = "Option::is_none"
	)]
	url: Option<url::Url>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	stars: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	forks: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	active: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	owner: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	commits: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	additions: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	deletions: Option<u64>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	languages: Vec<LanguageStat>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	tags: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	role: Option<ProjectRole>,
}

mod serde_option_display_fromstr {
	pub(crate) fn deserialize<'de, D, T>(deser: D) -> Result<Option<T>, D::Error>
	where
		D: serde::Deserializer<'de>,
		T: ::std::str::FromStr,
		<T as ::std::str::FromStr>::Err: ::std::fmt::Display,
	{
		#[derive(Default)]
		struct Visitor<T>(::std::marker::PhantomData<T>);
		impl<'de, T> serde::de::Visitor<'de> for Visitor<T>
		where
			T: ::std::str::FromStr,
			<T as ::std::str::FromStr>::Err: ::std::fmt::Display,
		{
			type Value = Option<T>;
			fn expecting(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
				write!(fmt, "a string")
			}
			fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
				v.parse()
					.map_err(serde::de::Error::custom)
					.map(Option::Some)
			}
		}

		deser.deserialize_str(Visitor::<T>(Default::default()))
	}

	pub(crate) fn serialize<S, T>(v: &Option<T>, ser: S) -> Result<S::Ok, S::Error>
	where
		S: serde::ser::Serializer,
		T: ::std::fmt::Display,
	{
		match v {
			Some(v) => ser.serialize_str(&v.to_string()),
			None => ser.serialize_none(),
		}
	}
}

use askama::Template;

struct ContactParams {
	value: String,
	icon: Option<String>,
	link: Option<String>,
}
#[derive(Template)]
#[template(path = "resume.html", escape = "none")]
struct ResumeParams<'a> {
	name: &'a str,
	resume_url: Option<&'a str>,
	contacts: Vec<ContactParams>,
	educations: &'a [Education],
	experiences: &'a [Experience],
	projects: Vec<Project>,
	references: Vec<(&'a str, &'a str)>,
	publications: Vec<(&'a str, Option<u32>)>,
	skills: &'a [Skill],
}

async fn fetch(mut person: Person) -> anyhow::Result<Person> {
	use futures::stream::TryStreamExt;
	let github_username = person
		.contacts
		.iter()
		.find(|v| v.type_ == "github")
		.map(|v| v.value.as_str());

	let mut project_map = HashMap::new();
	let mut sort_order = None;
	let mut import_mode = ProjectImportMode::Combine;
	// Process project imports first
	for pi in person.projects.iter() {
		match pi {
			ProjectParam::Import(ProjectImport::GitHub {
				ignore_forks,
				repos: None,
				token,
			}) => project_map.extend(
				github::get_user_projects_from_github(*ignore_forks, token.clone())
					.await?
					.into_iter()
					.map(|v| (v.name.clone(), v)),
			),
			ProjectParam::Import(ProjectImport::GitHub {
				repos: Some(repos),
				token,
				..
			}) => {
				project_map.extend(
					github::get_projects_info_from_github(
						repos,
						token.clone(),
						github_username.map(ToOwned::to_owned),
					)
					.await?
					.into_iter()
					.map(|v| (v.name.clone(), v)),
				);
			}
			ProjectParam::Sort { order_by } => {
				sort_order = Some(*order_by);
			}
			ProjectParam::ImportMode { import_mode: im } => {
				import_mode = *im;
			}
			_ => {}
		}
	}
	if sort_order.is_none() && import_mode == ProjectImportMode::Whitelist {
		sort_order = Some(ProjectSortOrder::Manual);
	}

	// Adding manually project entries
	for pi in person.projects.iter_mut() {
		match pi {
			ProjectParam::Raw(p) => {
				p.languages
					.sort_unstable_by_key(|v| ::std::cmp::Reverse(v.percentage));
				if let Some(old) = project_map.get_mut(&p.name) {
					debug!("Merging project entry {}", p.name);
					if p.url.is_some() {
						old.url = p.url.clone();
					}
					if p.description.is_some() {
						old.description = p.description.clone();
					}
					if p.owner.is_some() {
						old.owner = p.owner.clone();
					}
					if p.contributions.is_some() {
						old.contributions = p.contributions.clone();
					}
					if !p.tags.is_empty() {
						old.tags = p.tags.clone();
					}
					if p.role.is_some() {
						old.role = p.role;
					}
				} else {
					project_map.insert(p.name.clone(), p.clone());
				}
			}
			_ => {}
		}
	}
	let mut projects: Vec<_>;
	if let ProjectImportMode::Whitelist = import_mode {
		let raw_entries: Vec<_> = person
			.projects
			.iter()
			.filter_map(|p| match p {
				ProjectParam::Raw(p) => Some(&p.name),
				_ => None,
			})
			.collect();
		projects = raw_entries
			.into_iter()
			.filter_map(|name| project_map.get(name).map(Clone::clone))
			.collect();
	} else {
		projects = project_map.iter().map(|(_, v)| v.clone()).collect();
	}
	if let Some(sort_order) = sort_order {
		use ::std::cmp::Reverse;
		match sort_order {
			ProjectSortOrder::Stars => projects.sort_unstable_by_key(|v| Reverse(v.stars)),
			ProjectSortOrder::Forks => projects.sort_unstable_by_key(|v| Reverse(v.forks)),
			ProjectSortOrder::ForksThenStars => {
				projects.sort_unstable_by_key(|v| Reverse((v.forks, v.stars)))
			}
			ProjectSortOrder::StarsThenForks => {
				projects.sort_unstable_by_key(|v| Reverse((v.stars, v.forks)))
			}
			ProjectSortOrder::Manual if import_mode != ProjectImportMode::Whitelist => {
				debug!("Manual sort");
				let raw_entries: HashMap<_, _> = person
					.projects
					.iter()
					.filter_map(|p| match p {
						ProjectParam::Raw(p) => Some(&p.name),
						_ => None,
					})
					.enumerate()
					.map(|(i, v)| (v, i))
					.collect();
				projects.sort_unstable_by_key(|v| raw_entries.get(&v.name).map(|v| *v));
			}
			_ => {}
		}
	}
	debug!("{}", serde_yaml::to_string(&projects)?);
	person.projects = projects.into_iter().map(|v| ProjectParam::Raw(v)).collect();
	person.projects.push(ProjectParam::Sort {
		order_by: ProjectSortOrder::Manual,
	});

	// Fetch citations
	use ::futures::FutureExt;
	debug!("{:?}", person.references);
	let fut: futures::stream::FuturesUnordered<_> = person
		.references
		.iter_mut()
		.map(|(_, v)| v)
		.chain(person.publications.iter_mut())
		.map(|v| {
			async move {
				Result::<_, Error>::Ok(match v {
					Citation::Url(url) => url.fetch().await?,
					Citation::Doi(doi) => doi.fetch().await?,
					Citation::Bibtex(bib) => bib.fetch().await?,
					_ => (),
				})
			}
			.boxed()
		})
		.collect();
	let () = fut.try_collect().await?;
	person.references = person
		.references
		.into_iter()
		.filter_map(|(k, v)| v.to_raw().map(|v| (k, v)))
		.collect();
	person.publications = person
		.publications
		.into_iter()
		.filter_map(|v| v.to_raw_with_year())
		.collect();
	debug!("{:?}", person.references);
	Ok(person)
}

fn build_params<'a>(
	p: &'a Person,
	footnotes: Option<HashMap<String, usize>>,
) -> Result<ResumeParams<'a>, Error> {
	let mut c = Vec::new();
	let it = p.references.iter().filter_map(|(k, v)| match v {
		Citation::Raw(s) => Some((k.as_str(), s.as_str())),
		_ => None,
	});
	let mut references: Vec<_> = if let Some(footnotes) = footnotes.as_ref() {
		// Remove unused references
		it.filter(|(k, _)| footnotes.get(*k).is_some()).collect()
	} else {
		it.collect()
	};
	// Sort references
	if let Some(footnotes) = footnotes {
		references.sort_unstable_by_key(|(k, _)| footnotes.get(*k).unwrap());
	}

	for i in p.contacts.iter() {
		c.push(ContactParams {
			link: match i.type_.as_str() {
				"github" => Some(format!("https://github.com/{}", i.value)),
				"email" => Some(format!("mailto:{}", i.value)),
				"blog" => Some(i.value.clone()),
				_ => None,
			},
			icon: match i.type_.as_str() {
				"github" => Some("icons/github.svg".into()),
				"email" => Some("icons/mail.svg".into()),
				"blog" => Some("icons/blog.svg".into()),
				_ => None,
			},
			value: i.value.clone(),
		});
	}

	Ok(ResumeParams {
		name: &p.name,
		resume_url: p.resume_url.as_ref().map(String::as_str),
		contacts: c,
		educations: p.educations.as_slice(),
		experiences: p.experiences.as_slice(),
		projects: p
			.projects
			.iter()
			.filter_map(|v| match v {
				ProjectParam::Raw(p) => Some(p.clone()),
				_ => None,
			})
			.collect(),
		publications: p
			.publications
			.iter()
			.filter_map(|v| match v {
				Citation::RawWithYear { text, year } => Some((text.as_str(), *year)),
				_ => None,
			})
			.collect(),
		references,
		skills: p.skills.as_slice(),
	})
}

fn main() -> Result<(), Error> {
	env_logger::init();
	let args = clap::Command::new("resume")
		.arg(clap::Arg::new("input").required(true))
		.get_matches();
	let input_filename = args.get_one::<String>("input").unwrap();
	let cache_filename = format!("{}-cache", input_filename);
	let cache_info = std::fs::metadata(&cache_filename);
	let input_info = std::fs::metadata(&input_filename)?;
	let cache_data = if cache_info.is_ok() {
		std::fs::read(&cache_filename).ok()
	} else {
		None
	};
	let r = if cache_data.is_some() && cache_info?.modified()? >= input_info.modified()? {
		serde_yaml::from_slice::<Person>(cache_data.unwrap().as_slice())?
	} else {
		let f = std::fs::read(input_filename).unwrap();
		let r = serde_yaml::from_slice::<Person>(f.as_slice())?;
		debug!("{}", serde_yaml::to_string(&r)?);

		let mut runtime = tokio::runtime::Runtime::new()?;
		let r = runtime.block_on(fetch(r))?;
		if let Some(mut cache_f) =
			std::fs::File::create(format!("{}-cache", input_filename)).ok()
		{
			use ::std::io::Write;
			write!(cache_f, "{}", serde_yaml::to_string(&r)?)?;
		}
		r
	};
	let resume = build_params(&r, None)?;
	resume.render()?;

	let footnotes = FOOTNOTES.lock().unwrap().replace(HashMap::new()).unwrap();
	let resume = build_params(&r, Some(footnotes))?;
	println!("{}", resume.render()?);
	Ok(())
}
