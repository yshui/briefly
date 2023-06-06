use ::isahc::{prelude::*, Request};
use ::log::*;
use ::serde::{Deserialize, Serialize};
use ::std::collections::HashMap;
use ::std::fmt::Write;
use anyhow::Error;
use html5gum::{Token, Tokenizer};

#[serde_with::serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct UrlCitation {
	#[serde_as(as = "serde_with::DisplayFromStr")]
	url: ::url::Url,
	#[serde(skip)]
	title: Option<String>,
}

impl UrlCitation {
	pub(crate) async fn fetch(&mut self) -> Result<(), Error> {
		let mut response = isahc::get_async(self.url.as_str()).await?;
		let text = response.text().await?;
		let mut title_bytes = Vec::new();
		let mut title = None;
		let mut in_title = false;
		for token in Tokenizer::new(&text).infallible() {
			match token {
				Token::StartTag(tag) if tag.name.as_slice() == b"title" => {
					if title.is_none() {
						in_title = true;
					}
				}
				Token::String(s) => {
					if in_title {
						title_bytes.extend_from_slice(s.as_slice())
					}
				}
				Token::EndTag(tag) if tag.name.as_slice() == b"title" => {
					if title.is_none() {
						title = Some(String::from_utf8_lossy(&title_bytes).to_string());
						in_title = false;
					}
				}
				_ => (),
			}
		}
		self.title = title;
		Ok(())
	}

	pub(crate) fn to_raw(&self) -> Option<crate::Citation> {
		if let Some(title) = self.title.as_ref() {
			let mut ret = String::new();
			write!(&mut ret, r#"<b>{title}.</b> "#, title = title).unwrap();

			write!(
				&mut ret,
				r#"<a href="{url}">{url}</a>"#,
				url = self.url.as_str()
			)
			.unwrap();
			Some(crate::Citation::Raw(ret))
		} else {
			None
		}
	}

	pub(crate) fn year(&self) -> Option<u32> {
		None
	}
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct BibtexFileCitation {
	bibtex: ::std::path::PathBuf,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct BibtexCitation {
	bibtex_string: String,
	#[serde(skip)]
	tags: Option<HashMap<String, String>>,
}

impl BibtexCitation {
	pub(crate) async fn fetch(&mut self) -> Result<(), Error> {
		let data = nom_bibtex::Bibtex::parse(&self.bibtex_string)?;
		let data = data.bibliographies();
		if data.len() > 1 {
			warn!("more than 1 citations found in bibtex, only use the first one");
		}
		let data = data
			.get(0)
			.map(Ok)
			.unwrap_or(Err(anyhow::anyhow!("No citations found in bibtex")))?;
		self.tags = Some(
			data.tags()
				.iter()
				.map(|(k, v)| (k.to_owned(), v.to_owned()))
				.collect(),
		);
		Ok(())
	}
	pub(crate) fn to_raw(&self) -> Option<crate::Citation> {
		if let Some(tags) = self.tags.as_ref() {
			let mut ret = String::new();
			if let Some(author) = tags.get("author") {
				write!(&mut ret, "{}. ", author).unwrap();
			}
			if let Some(year) = tags.get("year") {
				write!(&mut ret, "{}. ", year).unwrap();
			}
			write!(&mut ret, "<b>{}.</b> ", tags.get("title").unwrap()).unwrap();
			if let Some(journal) = tags.get("journal") {
				write!(&mut ret, "In <i>{}</i>", journal).unwrap();
				if let Some(publisher) = tags.get("publisher") {
					write!(&mut ret, ", {}", publisher).unwrap();
				}
				write!(&mut ret, ". ").unwrap();
			}
			if let Some(doi) = tags.get("DOI") {
				write!(
					&mut ret,
					r#"DOI:<a href="{url}">{url}</a>"#,
					url = format!("https://doi.org/{}", doi)
				)
				.unwrap();
			} else if let Some(url) = tags.get("url") {
				write!(&mut ret, r#"<a href="{url}">{url}</a>"#, url = url).unwrap();
			}
			Some(crate::Citation::Raw(ret))
		} else {
			None
		}
	}
	pub(crate) fn year(&self) -> Option<u32> {
		self.tags
			.as_ref()
			.and_then(|v| v.get("year"))
			.map(|y| y.parse().unwrap())
	}
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct DoiCitation {
	doi: String,
	#[serde(skip)]
	csl: Option<Csl>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct CslDates {
	date_parts: Vec<Vec<u64>>,
}

#[derive(Deserialize, Serialize, Debug)]
struct CslAuthor {
	given: String,
	family: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct Csl {
	#[serde(default)]
	issued: Option<CslDates>,
	#[serde(rename = "URL", with = "crate::serde_option_display_fromstr")]
	url: Option<::url::Url>,
	#[serde(default)]
	container_title: Option<String>,
	author: Vec<CslAuthor>,
	title: String,
	#[serde(rename = "DOI", default)]
	doi: Option<String>,
	#[serde(default)]
	publisher: Option<String>,
}

impl DoiCitation {
	pub(crate) async fn fetch(&mut self) -> Result<(), Error> {
		let req = Request::builder()
			.uri(format!("https://doi.org/{}", self.doi))
			.header("Accept", "application/citeproc+json")
			.header("User-Agent", "curl/7.72.0")
			.redirect_policy(isahc::config::RedirectPolicy::Follow)
			.body(())?;
		let mut res = isahc::send_async(req).await?;
		let text = res.text().await?;
		debug!("Fetched {}", self.doi);
		debug!("{:?}", text);
		self.csl = Some(serde_json::from_str(&text)?);
		debug!("{:?}", self.csl);
		Ok(())
	}

	pub(crate) fn to_raw(&self) -> Option<crate::Citation> {
		use itertools::Itertools;
		if let Some(csl) = self.csl.as_ref() {
			let mut ret = String::new();

			if !csl.author.is_empty() {
				write!(
					&mut ret,
					"{}. ",
					csl.author
						.iter()
						.map(|a| format!("{} {}", a.given, a.family))
						.join(", ")
				)
				.unwrap();
			}
			if let Some(issued) = csl
				.issued
				.as_ref()
				.and_then(|i| i.date_parts.get(0))
				.and_then(|d| d.get(0))
			{
				write!(&mut ret, "{}. ", issued).unwrap();
			}
			write!(&mut ret, "<b>{}.</b> ", csl.title).unwrap();
			if let Some(journal) = csl.container_title.as_ref() {
				write!(&mut ret, "In <i>{}</i>", journal).unwrap();
				if let Some(publisher) = csl.publisher.as_ref() {
					write!(&mut ret, ", {}", publisher).unwrap();
				}
				write!(&mut ret, ". ").unwrap();
			}
			if let Some(doi) = csl.doi.as_ref() {
				write!(
					&mut ret,
					r#"DOI:<a href="{url}">{url}</a>"#,
					url = format!("https://doi.org/{}", doi)
				)
				.unwrap();
			} else if let Some(url) = csl.url.as_ref() {
				write!(&mut ret, r#"<a href="{url}">{url}</a>"#, url = url).unwrap();
			}
			Some(crate::Citation::Raw(ret))
		} else {
			None
		}
	}

	pub(crate) fn year(&self) -> Option<u32> {
		self.csl
			.as_ref()
			.and_then(|c| c.issued.as_ref())
			.and_then(|c| c.date_parts.get(0))
			.and_then(|c| c.get(0))
			.map(|c| *c as u32)
	}
}
