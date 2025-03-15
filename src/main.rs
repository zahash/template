/*
main.layout.html

<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="UTF-8">
    <title>Page Title</title>
</head>

<body>
    <header>Site Header</header>

    <placeholder name="sidebar" />
    <placeholder name="content" />

    <import fragment="something"/>

    <footer>Site Footer</footer>
</body>

</html>
*/

/*
main-content.fragment.html

<div>
    <p>This is the main content.</p>
</div>
*/

/*
home.page.html

<layout name = "main">
    <fill placeholder = "sidebar">
        <nav>
            this is the sidebar
        </nav>
    </fill>

    <fill placeholder = "content" fragment = "main-content" />

</layout>
*/

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use clap::Parser;
use scraper::{Html, Selector};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Files {
    pages: Vec<PathBuf>,
    layouts: HashMap<String, PathBuf>,
    fragments: HashMap<String, PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0:?}")]
    Io(#[from] io::Error),

    #[error("{0:?}")]
    TagNotFound(&'static str),

    #[error("{self:?}")]
    AttrNotFound {
        tag: &'static str,
        attr: &'static str,
    },

    #[error("{0:?}")]
    LayoutNotFound(String),

    #[error("{0:?}")]
    FragmentNotFound(String),
}

impl Files {
    pub fn walk<P>(root: P) -> Self
    where
        P: AsRef<Path>,
    {
        let mut files = Files {
            pages: Vec::new(),
            layouts: HashMap::new(),
            fragments: HashMap::new(),
        };

        for item in WalkDir::new(root) {
            if let Ok(entry) = item.inspect_err(|err| println!("{:?}", err)) {
                let path = entry.path();
                if path.is_file() && path.extension().map(|ext| ext == "html").unwrap_or(false) {
                    if let Some(filename) = path
                        .file_name()
                        .map(|osstr| osstr.to_string_lossy().to_string())
                    {
                        if let Some(_) = filename.strip_suffix(".page.html") {
                            files.pages.push(path.to_path_buf());
                        } else if let Some(layout_name) = filename.strip_suffix(".layout.html") {
                            files
                                .layouts
                                .insert(layout_name.to_string(), path.to_path_buf());
                        } else if let Some(fragment_name) = filename.strip_suffix(".fragment.html")
                        {
                            files
                                .fragments
                                .insert(fragment_name.to_string(), path.to_path_buf());
                        }
                    }
                }
            }
        }

        files
    }

    fn render_page(&self, page: Html) -> Result<String, Error> {
        let Some(layout_tag) = page
            .select(&Selector::parse("layout").expect("invalid selector `layout`"))
            .next()
        else {
            return Err(Error::TagNotFound("layout"));
        };

        let Some(layout_name) = layout_tag.value().attr("name") else {
            return Err(Error::AttrNotFound {
                tag: "layout",
                attr: "name",
            });
        };

        let Some(layout_path) = self.layouts.get(layout_name) else {
            return Err(Error::LayoutNotFound(layout_name.to_string()));
        };

        let mut layout_content = fs::read_to_string(layout_path)?;

        for fill_tag in page.select(&Selector::parse("fill").expect("invalid selector `fill`")) {
            let Some(placeholder_name) = fill_tag.value().attr("placeholder") else {
                return Err(Error::AttrNotFound {
                    tag: "fill",
                    attr: "placeholder",
                });
            };

            let fragment_content = match fill_tag.value().attr("fragment") {
                Some(fragment_name) => {
                    let Some(fragment_filepath) = self.fragments.get(fragment_name) else {
                        return Err(Error::FragmentNotFound(fragment_name.to_string()));
                    };
                    fs::read_to_string(fragment_filepath)?
                }
                None => fill_tag.html(),
            };

            layout_content = layout_content.replace(
                &format!(r#"<placeholder name="{}" />"#, placeholder_name),
                &fragment_content,
            );
        }

        Ok(layout_content)
    }

    pub fn render<P>(&self, dist: P) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        for page_path in &self.pages {
            let page_content = fs::read_to_string(page_path)?;
            let page = Html::parse_document(&page_content);
            let rendered_page = self.render_page(page)?;

            let dist_path = dist.as_ref().join(page_path);
            let dist_folder = dist_path
                .parent()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid output path"))?;

            fs::create_dir_all(dist_folder)?;
            fs::write(&dist_path, rendered_page)?;
        }

        Ok(())
    }
}

#[derive(Debug, Parser)]
#[command(version, about = "Render HTML pages using layouts and fragments")]
struct Args {
    /// Source folder containing HTML templates
    #[arg(short, long)]
    source: PathBuf,

    /// Destination folder for the rendered pages
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let files = Files::walk(&args.source);
    files.render(&args.output)?;

    Ok(())
}
