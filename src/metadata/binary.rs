use std::fs::File;
use std::io::Read;
use std::path::Path;
use super::model::MetadataSection;

pub fn analyze_binary_metadata(path: &Path) -> Vec<MetadataSection> {
    let mut sections = Vec::new();
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return sections,
    };

    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buffer = Vec::new();
    let max_read = if file_len <= 64 * 1024 * 1024 {
        file_len
    } else {
        4 * 1024 * 1024
    };
    let mut take = (&mut file).take(max_read);
    if take.read_to_end(&mut buffer).is_err() || buffer.len() < 4 {
        return sections;
    }

    let parsed = match goblin::Object::parse(&buffer) {
        Ok(obj) => obj,
        Err(_) => return sections,
    };

    let mut bin_sec = MetadataSection::new("Binary & Executable Info");
    let mut deps_sec = MetadataSection::new("Linked Libraries & Dependencies");

    match parsed {
        goblin::Object::Elf(elf) => {
            bin_sec.add("Format", "ELF (Executable and Linkable Format)");
            bin_sec.add("Bitness", if elf.is_64 { "64-bit" } else { "32-bit" });
            bin_sec.add("Endianness", if elf.little_endian { "Little Endian" } else { "Big Endian" });
            bin_sec.add("Architecture", format!("{:#x}", elf.header.e_machine));
            bin_sec.add("Entry Point", format!("{:#018x}", elf.entry));
            bin_sec.add("Sections Count", elf.section_headers.len().to_string());
            if let Some(interp) = elf.interpreter {
                bin_sec.add("Dynamic Interpreter", interp);
            }
            bin_sec.add("Position Independent (PIE)", if elf.is_lib { "Yes (PIE / Shared Lib)" } else { "No (Fixed Address)" });

            for lib in elf.libraries {
                deps_sec.add("Shared Library", lib);
            }
        }
        goblin::Object::PE(pe) => {
            bin_sec.add("Format", "PE / COFF (Windows Portable Executable)");
            bin_sec.add("Bitness", if pe.is_64 { "64-bit" } else { "32-bit" });
            bin_sec.add("Machine", format!("{:#x}", pe.header.coff_header.machine));
            bin_sec.add("Sections Count", pe.sections.len().to_string());

            if let Some(opt) = pe.header.optional_header {
                let subsystem_str = match opt.windows_fields.subsystem {
                    2 => "Windows GUI",
                    3 => "Windows Console (CUI)",
                    7 => "POSIX CUI",
                    9 => "Windows CE GUI",
                    10 => "EFI Application",
                    _ => "Other",
                };
                bin_sec.add("Subsystem", subsystem_str);
                bin_sec.add("Image Base", format!("{:#018x}", opt.windows_fields.image_base));
                bin_sec.add("Entry Point RVA", format!("{:#010x}", opt.standard_fields.address_of_entry_point));
            }

            for lib in pe.libraries {
                deps_sec.add("Imported DLL", lib);
            }
        }
        goblin::Object::Mach(mach) => match mach {
            goblin::mach::Mach::Binary(macho) => {
                bin_sec.add("Format", "Mach-O (macOS / iOS / Darwin Binary)");
                bin_sec.add("Bitness", if macho.is_64 { "64-bit" } else { "32-bit" });
                bin_sec.add("CPU Type", format!("{:#x}", macho.header.cputype));
                bin_sec.add("CPU Subtype", format!("{:#x}", macho.header.cpusubtype));
                bin_sec.add("Load Commands", macho.header.ncmds.to_string());
                bin_sec.add("Entry Point", format!("{:#018x}", macho.entry));

                for lib in macho.libs {
                    deps_sec.add("Linked Dylib", lib);
                }
            }
            goblin::mach::Mach::Fat(fat) => {
                bin_sec.add("Format", "Universal / Fat Mach-O Binary");
                bin_sec.add("Architectures Count", fat.narches.to_string());
                for (idx, arch) in fat.iter_arches().enumerate() {
                    if let Ok(a) = arch {
                        bin_sec.add(format!("Slice #{}", idx + 1), format!("cputype: {:#x}, cpusubtype: {:#x}", a.cputype, a.cpusubtype));
                    }
                }
            }
        },
        goblin::Object::Archive(archive) => {
            bin_sec.add("Format", "Static Library Archive (.a / .lib)");
            bin_sec.add("Members Count", archive.len().to_string());
        }
        _ => {}
    }

    if !bin_sec.entries.is_empty() {
        sections.push(bin_sec);
    }
    if !deps_sec.entries.is_empty() {
        sections.push(deps_sec);
    }

    sections
}
