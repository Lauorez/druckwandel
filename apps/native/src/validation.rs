use serde::Deserialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_REPORT_BYTES: u64 = 10 * 1024 * 1024;
const MAX_XML_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    java: Java,
    kosit: Kosit,
    mustang: Jar,
    verapdf: Jar,
}

#[derive(Deserialize)]
struct Jar {
    jar: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Java {
    relative_path: String,
}

#[derive(Deserialize)]
struct Kosit {
    jar: String,
    scenarios: String,
    repository: String,
}

pub struct ValidatorBundle {
    java: PathBuf,
    kosit: PathBuf,
    scenarios: PathBuf,
    repository: PathBuf,
    mustang: PathBuf,
    verapdf: PathBuf,
}

impl ValidatorBundle {
    pub fn from_root(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("Prüfpaket fehlt: {e}"))?;
        let manifest: Manifest = serde_json::from_slice(
            &fs::read(root.join("manifest.json")).map_err(|e| format!("Prüfpaket fehlt: {e}"))?,
        )
        .map_err(|e| format!("Prüfpaket ist beschädigt: {e}"))?;
        let mut java = root.join(manifest.java.relative_path);
        if cfg!(windows) && java.extension().is_none() {
            java.set_extension("exe");
        }
        let bundle = Self {
            java,
            kosit: root.join(manifest.kosit.jar),
            scenarios: root.join(manifest.kosit.scenarios),
            repository: root.join(manifest.kosit.repository),
            mustang: root.join(manifest.mustang.jar),
            verapdf: root.join(manifest.verapdf.jar),
        };
        for path in [
            &bundle.java,
            &bundle.kosit,
            &bundle.scenarios,
            &bundle.repository,
            &bundle.mustang,
            &bundle.verapdf,
        ] {
            if !path.exists() {
                return Err(format!("Teil des Prüfpakets fehlt: {}", path.display()));
            }
        }
        Ok(bundle)
    }

    pub fn discover() -> Result<Self, String> {
        if let Some(root) = std::env::var_os("ERECHNUNG_VALIDATOR_ROOT") {
            return Self::from_root(Path::new(&root));
        }
        if let Ok(executable) = std::env::current_exe() {
            if let Some(parent) = executable.parent() {
                let packaged = parent.join("resources").join("validators");
                if packaged.exists() {
                    return Self::from_root(&packaged);
                }
                let mac_bundle = parent.join("../Resources/validators");
                if mac_bundle.exists() {
                    return Self::from_root(&mac_bundle);
                }
            }
        }
        let development = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/validators");
        Self::from_root(&development)
    }

    pub fn validate_xrechnung(&self, xml: &[u8]) -> Result<(), String> {
        if xml.is_empty() || xml.len() > MAX_XML_BYTES {
            return Err("Ungültige Größe der XML-Ausgabe.".into());
        }
        let work = tempfile::tempdir().map_err(|e| e.to_string())?;
        let input = work.path().join("invoice.xml");
        let reports = work.path().join("reports");
        let log_path = work.path().join("validator.log");
        fs::create_dir(&reports).map_err(|e| e.to_string())?;
        fs::write(&input, xml).map_err(|e| e.to_string())?;
        let log = File::create(&log_path).map_err(|e| e.to_string())?;
        let error_log = log.try_clone().map_err(|e| e.to_string())?;
        let mut command = Command::new(&self.java);
        command
            .args([
                "-Djava.awt.headless=true",
                "-Djava.net.useSystemProxies=false",
                "-Dhttp.proxyHost=127.0.0.1",
                "-Dhttp.proxyPort=9",
                "-Dhttps.proxyHost=127.0.0.1",
                "-Dhttps.proxyPort=9",
                "-Djavax.xml.accessExternalDTD=",
                "-Djavax.xml.accessExternalSchema=file,jar:file",
                "-Djavax.xml.accessExternalStylesheet=",
                "-jar",
            ])
            .arg(&self.kosit)
            .arg("-s")
            .arg(&self.scenarios)
            .arg("-r")
            .arg(&self.repository)
            .arg("-o")
            .arg(&reports)
            .arg(&input)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(error_log));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("KoSIT konnte nicht gestartet werden: {e}"))?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if started.elapsed() >= TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err("KoSIT hat die Zeitgrenze überschritten.".into());
            }
            thread::sleep(Duration::from_millis(50));
        };
        let report_path = fs::read_dir(&reports)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with("-report.xml"))
            })
            .ok_or_else(|| {
                let mut details = String::new();
                if let Ok(file) = File::open(&log_path) {
                    let _ = file.take(8192).read_to_string(&mut details);
                }
                format!(
                    "KoSIT hat keinen Prüfbericht erzeugt (Exit-Code: {:?}): {}",
                    status.code(),
                    details.trim()
                )
            })?;
        if fs::metadata(&report_path).map_err(|e| e.to_string())?.len() > MAX_REPORT_BYTES {
            return Err("Der KoSIT-Prüfbericht ist zu groß.".into());
        }
        let report = fs::read_to_string(report_path).map_err(|e| e.to_string())?;
        let doc = roxmltree::Document::parse(&report)
            .map_err(|e| format!("KoSIT-Prüfbericht ist beschädigt: {e}"))?;
        let root = doc
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "report")
            .ok_or("KoSIT-Prüfbericht enthält kein Ergebnis.")?;
        let has_assessment = root
            .descendants()
            .any(|node| node.is_element() && node.tag_name().name() == "assessment");
        let has_accept = root
            .descendants()
            .any(|node| node.is_element() && node.tag_name().name() == "accept");
        let accepted = root.attribute("valid") == Some("true")
            && !root
                .descendants()
                .any(|node| node.is_element() && node.tag_name().name() == "reject")
            && (!has_assessment || has_accept)
            && status.success();
        if accepted {
            return Ok(());
        }
        let issues: Vec<_> = root
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "failed-assert")
            .filter_map(|node| {
                node.descendants()
                    .find(|child| child.is_element() && child.tag_name().name() == "text")
            })
            .filter_map(|node| node.text())
            .take(3)
            .collect();
        if issues.is_empty() {
            Err("KoSIT hat die XRechnung abgelehnt.".into())
        } else {
            Err(format!(
                "KoSIT hat die XRechnung abgelehnt: {}",
                issues.join("; ")
            ))
        }
    }
}

impl ValidatorBundle {
    fn java_command(&self) -> Command {
        let mut command = Command::new(&self.java);
        command.args([
            "-Djava.awt.headless=true",
            "-Djava.net.useSystemProxies=false",
            "-Dhttp.proxyHost=127.0.0.1",
            "-Dhttp.proxyPort=9",
            "-Dhttps.proxyHost=127.0.0.1",
            "-Dhttps.proxyPort=9",
            "-Djavax.xml.accessExternalDTD=",
            "-Djavax.xml.accessExternalSchema=file,jar:file",
            "-Djavax.xml.accessExternalStylesheet=",
        ]);
        command
    }

    pub fn validate_cii(&self, xml: &[u8]) -> Result<(), String> {
        if xml.is_empty() || xml.len() > MAX_XML_BYTES {
            return Err("Ungültige Größe der CII-Datei.".into());
        }
        let work = tempfile::tempdir().map_err(|e| e.to_string())?;
        let input = work.path().join("invoice.xml");
        fs::write(&input, xml).map_err(|e| e.to_string())?;
        let mut command = self.java_command();
        command
            .arg("-jar")
            .arg(&self.mustang)
            .args(["--action", "validate", "--source"])
            .arg(input)
            .arg("--disable-file-logging");
        let (success, output) = run_java(command, work.path())?;
        evaluate_mustang(success, &output)
    }

    pub fn validate_zugferd(&self, pdf: &[u8], expected_xml: &[u8]) -> Result<(), String> {
        self.validate_cii(expected_xml)?;
        let work = tempfile::tempdir().map_err(|e| e.to_string())?;
        let input = work.path().join("invoice.pdf");
        let extracted = work.path().join("extracted.xml");
        fs::write(&input, pdf).map_err(|e| e.to_string())?;
        let mut command = self.java_command();
        command
            .arg("-jar")
            .arg(&self.mustang)
            .args(["--action", "extract", "--source"])
            .arg(&input)
            .arg("--out")
            .arg(&extracted)
            .arg("--disable-file-logging");
        let (success, _) = run_java(command, work.path())?;
        if !success
            || fs::read(&extracted)
                .map_err(|e| format!("Mustang konnte die Rechnungsdaten nicht auslesen: {e}"))?
                != expected_xml
        {
            return Err(
                "Die aus der PDF gelesenen Rechnungsdaten weichen von der geprüften CII-Datei ab."
                    .into(),
            );
        }
        let mut command = self.java_command();
        command
            .arg("-cp")
            .arg(&self.verapdf)
            .args([
                "org.verapdf.apps.GreenfieldCliWrapper",
                "--flavour",
                "3b",
                "--format",
                "xml",
                "--maxfailures",
                "20",
                "--loglevel",
                "0",
            ])
            .arg(&input);
        let (success, output) = run_java(command, work.path())?;
        evaluate_verapdf(success, &output)
    }
}

fn run_java(mut command: Command, work: &Path) -> Result<(bool, String), String> {
    let log_path = work.join(format!("{}.log", uuid::Uuid::new_v4()));
    let log = File::create(&log_path).map_err(|e| e.to_string())?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Prüfer konnte nicht gestartet werden: {e}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() >= TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Die Prüfung hat die Zeitgrenze überschritten.".into());
        }
        thread::sleep(Duration::from_millis(50));
    };
    if fs::metadata(&log_path).map_err(|e| e.to_string())?.len() > MAX_REPORT_BYTES {
        return Err("Prüfbericht ist zu groß.".into());
    }
    Ok((
        status.success(),
        fs::read_to_string(log_path).map_err(|e| e.to_string())?,
    ))
}

fn report_fragment<'a>(output: &'a str, root: &str) -> Result<&'a str, String> {
    let start = output
        .find(&format!("<{root}"))
        .ok_or("Maschinenlesbarer Prüfbericht fehlt.")?;
    let closing = format!("</{root}>");
    let end = output[start..]
        .find(&closing)
        .ok_or("Prüfbericht ist unvollständig.")?;
    Ok(&output[start..start + end + closing.len()])
}

fn evaluate_mustang(success: bool, output: &str) -> Result<(), String> {
    let fragment = report_fragment(output, "validation")?;
    let doc = roxmltree::Document::parse(fragment).map_err(|e| e.to_string())?;
    let status = doc
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "summary")
        .filter_map(|node| node.attribute("status"))
        .next_back();
    if success && status == Some("valid") {
        return Ok(());
    }
    let messages: Vec<_> = doc
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "error")
        .filter_map(|node| node.text())
        .take(3)
        .collect();
    Err(format!(
        "Mustang hat die Rechnungsdaten abgelehnt: {}",
        messages.join("; ")
    ))
}

fn evaluate_verapdf(success: bool, output: &str) -> Result<(), String> {
    let fragment = report_fragment(output, "report")?;
    let doc = roxmltree::Document::parse(fragment).map_err(|e| e.to_string())?;
    let reports: Vec<_> = doc
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "validationReport")
        .collect();
    let valid = !reports.is_empty()
        && reports.iter().all(|node| {
            node.attribute("isCompliant") == Some("true")
                && (matches!(node.attribute("flavour"), Some("3b" | "3B" | "PDF/A-3B"))
                    || node.attribute("profileName") == Some("PDF/A-3B validation profile"))
        });
    let failed = doc.descendants().any(|node| {
        node.is_element()
            && (node
                .attribute("failedChecks")
                .is_some_and(|value| value != "0")
                || node
                    .attribute("failedToParse")
                    .is_some_and(|value| value != "0")
                || node
                    .attribute("encrypted")
                    .is_some_and(|value| value != "0"))
    });
    if success && valid && !failed {
        Ok(())
    } else {
        Err("veraPDF hat die PDF/A-3B-Prüfung abgelehnt. Bitte eine PDF mit eingebetteten Schriften und ohne Formulare verwenden.".into())
    }
}
