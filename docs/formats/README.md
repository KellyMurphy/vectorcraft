# File format specifications

VectorCraft's own file formats are open: anyone may read and write them, in any program, under any
licence. This folder holds their specifications. Each format has a page describing it and, where it is
JSON, a [JSON Schema](https://json-schema.org/) (draft 2020-12) for checking files by machine. Tests in
the repository read these schemas and examples, so they stay in step with the code.

| Format | Extension | Spec | Schema |
|---|---|---|---|
| Border pattern (embroidery) | `.vcborder` | [vcborder.md](vcborder.md) | [vcborder.schema.json](vcborder.schema.json) |
| Embroidery font | `.vcfont` | [vcfont.md](vcfont.md) | [vcfont.schema.json](vcfont.schema.json) |
| Fabric profile (embroidery) | `.vcfabric` | [vcfabric.md](vcfabric.md) | [vcfabric.schema.json](vcfabric.schema.json) |
| Hoop list (embroidery) | `.vchoop` | [vchoop.md](vchoop.md) | [vchoop.schema.json](vchoop.schema.json) |
| Thread chart (embroidery) | `.vcthreads` | [vcthreads.md](vcthreads.md) | [vcthreads.schema.json](vcthreads.schema.json) |
| Embroidery data in documents | inside `.vectorcraft` | [vectorcraft-embroidery.md](vectorcraft-embroidery.md) | [vectorcraft-embroidery.schema.json](vectorcraft-embroidery.schema.json) |

The native document format, `.vectorcraft`, is described in the `vectorcraft-format` crate's
documentation (`crates/format/src/lib.rs`); its embroidery data is specified here.

Formats VectorCraft reads and writes but doesn't own (SVG, PDF, Tajima DST and the rest) follow their
own published specifications and aren't described here.

## What every format here has in common

- **JSON, UTF-8.** Files are plain text that any JSON library reads.
- **A header naming the format and its version:** `"format"` (a fixed string) and `"version"` (a whole
  number). A reader refuses a file whose `version` is newer than it knows, with a message, rather than
  read it wrongly. A new version is made only when an older reader would misread a file; adding an
  optional field that older readers can ignore doesn't need one.
- **Unknown fields are kept** by readers and written back when they save, so later additions neither break
  older programs nor get lost when an older program saves the file. Values a reader can't understand are
  kept as written too (each spec says how they are treated).
- **Millimetres** for embroidery lengths, the unit embroidery machines and their files work in.
- **Untrusted input.** Readers check every value (sizes, counts, finite numbers) and refuse a file that
  fails, never crash on it. The limits are in each spec.
- **Credits travel with the data:** formats meant to be shared carry author, licence and source fields.

## Adding a format

Every new file format VectorCraft defines gets a page here (and a schema, for JSON) in the same change
that adds it, plus a test that reads the schema and an example file and checks them against the code.
