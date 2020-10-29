use ::pulldown_cmark::{Parser, Options};
use crate::md::html;

use crate::FOOTNOTES;

pub(crate) fn md(s: &str) -> ::askama::Result<String> {
	let p = Parser::new_ext(s, Options::ENABLE_FOOTNOTES);
	let mut ret = String::new();
	let mut footnotes = FOOTNOTES.lock().unwrap().take().unwrap();
	html::push_html(&mut ret, p, &mut footnotes);
	*FOOTNOTES.lock().unwrap() = Some(footnotes);
	Ok(ret)
}

pub(crate) fn language_stats(languages: &[crate::LanguageStat]) -> ::askama::Result<String> {
	use ::std::fmt::Write;
	let mut percentage: crate::Decimal1 = 0f64.into();
	let mut languages_to_show = languages;
	for i in 0..languages.len() {
		if percentage > 95f64.into() {
			languages_to_show = &languages[..i];
			break;
		}
		percentage += languages[i].percentage;
	}

	let mut ret = String::new();
	ret.push_str(r#"<div class="language_bar">"#);
	for (i, l) in languages_to_show.iter().enumerate() {
		write!(
			&mut ret,
			r#"<span style="width:{}%;opacity:{}"></span>"#,
			l.percentage,
			(-0.6 * (i as f64)).exp()
		)?;
	}
	if percentage < 100f64.into() {
		write!(
			&mut ret,
			r#"<span style="width:{}%;opacity:{};border-right:0px"></span>"#,
			100.0 - <crate::Decimal1 as Into<f64>>::into(percentage),
			(-0.6 * languages_to_show.len() as f64).exp()
		)?;
	}
	ret.push_str("</div>");
	ret.push_str(r#"<div class="language_dots">"#);
	for (i, l) in languages_to_show.iter().enumerate() {
		write!(
			&mut ret,
			r#"<div class="language_dot"><span class="dot" style="opacity:{}"></span>"#,
			(-0.6 * (i as f64)).exp()
		)?;
		write!(&mut ret, "<span>{}</span></div>", l.language)?;
	}
	if percentage < 100f64.into() {
		write!(
			&mut ret,
			r#"<div class="language_dot"><span class="dot" style="opacity:{}"></span>"#,
			(-0.6 * languages_to_show.len() as f64).exp()
		)?;
		write!(&mut ret, "<span>Other</span></div>")?;
	}
	ret.push_str("</div>");
	Ok(ret)
}

pub(crate) fn emph(s: &str, pat: &str) -> ::askama::Result<String> {
	Ok(s.replace(pat, &format!("<u>{}</u>", pat)))
}
