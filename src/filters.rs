use crate::md::html;
use ::pulldown_cmark::{Options, Parser};

use crate::FOOTNOTES;

pub(crate) fn md(s: impl AsRef<str>) -> ::askama::Result<String> {
	let p = Parser::new_ext(s.as_ref(), Options::ENABLE_FOOTNOTES);
	let mut ret = String::new();
	let mut footnotes = FOOTNOTES.lock().unwrap().take().unwrap();
	html::push_html(&mut ret, p, &mut footnotes, false);
	*FOOTNOTES.lock().unwrap() = Some(footnotes);
	Ok(ret)
}

pub(crate) fn inline_md(s: impl AsRef<str>) -> ::askama::Result<String> {
	let p = Parser::new_ext(s.as_ref(), Options::ENABLE_FOOTNOTES);
	let mut ret = String::new();
	let mut footnotes = FOOTNOTES.lock().unwrap().take().unwrap();
	html::push_html(&mut ret, p, &mut footnotes, true);
	*FOOTNOTES.lock().unwrap() = Some(footnotes);
	Ok(ret)
}

fn opacity(index: usize, total: usize) -> f64 {
	let percentage = if index == 0 {
		// in case total == 1
		0.
	} else {
		(index as f64) / (total as f64 - 1.)
	};
	2.0 - (1.8f64.ln() * percentage).exp()
}

pub(crate) fn language_stats(languages: &[crate::LanguageStat]) -> ::askama::Result<String> {
	use ::std::fmt::Write;
	let mut percentage: crate::Decimal1 = 0f64.into();
	const THRESHOLD: f64 = 97.0;
	let mut languages_to_show = None;
	let max_languages = languages.len().min(4);
	for (i, p) in languages[..max_languages].iter().enumerate() {
		if percentage > THRESHOLD.into() {
			languages_to_show = Some(&languages[..i]);
			break;
		}
		percentage += p.percentage;
	}
	let languages_to_show = languages_to_show.unwrap_or(&languages[..max_languages]);

	let total = languages_to_show.len() + if percentage < 100f64.into() { 1 } else { 0 };
	let mut ret = String::new();
	ret.push_str(r#"<div class="language_bar">"#);
	for (i, l) in languages_to_show.iter().enumerate() {
		let is_last = (i == languages_to_show.len() - 1) && percentage >= 100f64.into();
		let border = if !is_last {
			"border-right: 1px solid white;"
		} else {
			""
		};
		write!(
			&mut ret,
			r#"<span style="width:{}%;opacity:{};{border}"></span>"#,
			l.percentage,
			opacity(i, total)
		)?;
	}
	if percentage < 100f64.into() {
		write!(
			&mut ret,
			r#"<span style="width:{}%;opacity:{};"></span>"#,
			100.0 - <crate::Decimal1 as Into<f64>>::into(percentage),
			opacity(total - 1, total)
		)?;
	}
	ret.push_str("</div>");
	ret.push_str(r#"<div class="language_dots">"#);
	for (i, l) in languages_to_show.iter().enumerate() {
		write!(
			&mut ret,
			r#"<div class="language_dot"><span class="dot" style="opacity:{}"></span>"#,
			opacity(i, total)
		)?;
		write!(&mut ret, "<span>{}</span></div>", l.language)?;
	}
	if percentage < 100f64.into() {
		write!(
			&mut ret,
			r#"<div class="language_dot"><span class="dot" style="opacity:{}"></span>"#,
			opacity(total - 1, total)
		)?;
		write!(&mut ret, "<span>Other</span></div>")?;
	}
	ret.push_str("</div>");
	Ok(ret)
}

pub(crate) fn emph(s: &str, pat: &str) -> ::askama::Result<String> {
	Ok(s.replace(pat, &format!("<u>{}</u>", pat)))
}
