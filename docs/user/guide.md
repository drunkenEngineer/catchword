# Catchword user guide

*Draft, for version 0.1. It describes the app as it is on 5 October 2026.*

Catchword finds passages in your own documents. You can describe what you are looking for in your own words, or type an exact word, name or number. It shows you the file and the place in it. Everything happens on your computer: see [What leaves your computer](network.md).

## Contents

1. [Installing](#installing)
2. [The first start](#the-first-start)
3. [Searching](#searching)
4. [When nothing is found](#when-nothing-is-found)
5. [Library: your folders and indexing](#library-your-folders-and-indexing)
6. [Settings](#settings)
7. [When something goes wrong](#when-something-goes-wrong)
8. [Uninstalling](#uninstalling)
9. [What Catchword can and cannot read yet](#what-catchword-can-and-cannot-read-yet)

## Installing

Catchword runs on Windows 11. Windows 10 should work, but is not tested yet. It needs no administrator rights: it installs for you alone, in `%LOCALAPPDATA%\Catchword`.

- **From GitHub:** run `Catchword_<version>_x64-setup.exe`. The download is not signed yet, so Windows may show "Windows protected your PC". Choose **More info**, then **Run anyway**.
- **From the Microsoft Store:** not available yet.

## The first start

Catchword asks two things.

1. **The promise.** Your files never leave this computer. Nothing about your documents or your searches is sent anywhere, and your files are never changed, moved or deleted. Choose **Continue**.
2. **Your folders.** Choose **Add a folder** and pick a folder that holds your documents, such as Documents. Add as many as you like, then choose **Start searching**. You can also skip this and add folders later, in Library.

Catchword then reads your folders. Some things are left out by default:

- system files;
- development folders such as `node_modules`;
- files that often hold passwords or keys, such as password-manager files, `*.pem` or `id_rsa*`.

You can change this in Settings.

## Searching

Type in the search box. Results appear as you type. You can:

- **describe what you want:** *the letter about my tax refund*, *notice period in the lease*;
- **type exact words, names or numbers:** *INV-2024-117*, *Raman*;
- **put words in quotes** to find them exactly as written: *"notice period"*.

A search in one language can find passages in another, for example English words that find a French letter.

Each result is one file, with its best passages beneath it. Each passage shows where it is (a page, or lines), and how it was found:

- **matched words:** your words are in it;
- **matched meaning:** it says what you described, perhaps in other words;
- **matched words and meaning:** both;
- **matched the file or folder name:** for example *plumber invoice* finds `Plumber\invoice-march.pdf`.

Results also show when each file last changed. If you have identical copies of a file, they appear once, with "and 1 identical copy".

### The preview

Choose a passage to see it in context on the right. From there:

- **Open** opens the file in its usual program;
- **Show in folder** opens its folder, with the file selected;
- **Copy passage** copies the passage, with the file's name and place;
- **Copy path** copies where the file is.

If a file was moved, renamed or deleted since Catchword last looked, it says so and offers **Scan now**.

### Filters

Above the results, you can keep to:

- **Folder:** one of your folders;
- **Kind:** PDF, or text and Markdown;
- **Changed:** files changed in the past week, month or year.

### Keyboard

| Action | Keys |
| --- | --- |
| Go to the search box | Ctrl+K or Ctrl+L |
| Move through the results | Up, Down |
| Show only a file's best passage, or all of them | Left, Right |
| Open the file | Enter |
| Show it in its folder | Ctrl+Enter |
| Copy the passage | Ctrl+C |
| Copy the file's path | Ctrl+Shift+C |
| Move between the search box, the results and the preview | F6 (Shift+F6 goes back) |
| Look for new and changed files now | F5 |
| Search, Library, Settings | Ctrl+1, Ctrl+2, Ctrl+3 |
| Clear the search | Esc |

### While Catchword is still reading

You can search at once. While your folders are read, Search says how far it has got: first your files become searchable by their words, then by their meaning. Until both are done, results may be incomplete.

Searching by meaning is the slow part. On a 2023 laptop, Catchword makes about 17 passages a second searchable by meaning, so a large library takes a few hours. Searching by words works meanwhile.

## When nothing is found

Catchword says why your document might be missing, with counts. For example:

- *3 files are scans with no text layer*;
- *1 file is protected by a password*;
- *2 files are kept only in the cloud, so they were not read*.

If filters were on, **Search everything** searches again without them. **Open Library** lists every file that was not read, with the reason.

## Library: your folders and indexing

### Folders

- **Add a folder** to search it too.
- **Scan now** (or F5) looks for new, changed, moved and deleted files at once. Catchword also looks each time it starts.
- **Remove** stops searching a folder and removes its text from the index. Your files are not touched.

A folder on a drive that is not connected is shown as not reachable. Its files stay searchable, but cannot be opened until the drive is back.

### Progress

Library shows both stages: how many files are searchable by words, and how many passages by meaning. While Catchword works, it shows how fast it goes and about how long is left, and when it is done, the time of the last scan.

### Pausing

Choose **Pause** to stop indexing, and **Resume** to carry on. Nothing done so far is lost. Indexing also pauses by itself, and Library says why:

- **on battery:** it carries on when you plug the computer in (you can turn this off in Settings);
- **low disk space:** less than 1 GB is free on the drive that holds the index;
- **the index's drive is not connected,** if you moved the index there;
- **after two unexpected closes in a row:** see [When something goes wrong](#when-something-goes-wrong).

Indexing always runs at low priority, so your other work comes first.

### Needs attention

Every file that was not read is listed here, with the reason:

- *no text layer (a scan?)*: it needs text recognition, which is not available yet;
- *protected by a password*;
- *over the size, page or text limit*: see Settings;
- *could not be opened*: access denied, or in use;
- *only in the cloud*: not downloaded, so not read;
- damaged files, and files that took too long or needed too much memory to read.

A file skipped for a reason such as a password or a size limit is not read again until it changes. Files that could not be opened, and files kept only in the cloud, are tried again at each scan. A file whose reading failed gets a second try; after that, choose **Try again** to read it once more.

## Settings

### What to leave out

- **Leave out a folder…** skips a folder inside one of yours, such as a private one. **Include again** undoes it.
- **Names to leave out:** one per line. `*` stands for any characters and `?` for one, so `*.tmp` leaves out every `.tmp` file. **Restore the defaults** brings back the original list.

What you leave out is also removed from the index.

### Appearance

Colours (as Windows is set, light or dark) and text size (normal, 115% or 130%).

### Indexing

- **How much of the processor indexing may use:**
  - **Light:** one core; best on battery or on an older computer;
  - **Balanced:** half the processor, the default;
  - **Fast:** all cores but one; the computer may get warm and loud.
- **Pause indexing while the computer runs on battery:** on by default.
- **Largest file to read** (200 MB by default) and **most pages to read from a PDF** (5,000 by default). Files skipped as too large are read again if they now fit.

### Your data

- **Where the index is,** and how large it is. The index holds the text of your documents, so treat it like them.
- **Check the index** looks for damage.
- **Move the index…** puts it in a folder you choose, for example on an encrypted drive. Before moving, Catchword tells you that uninstalling will not remove it from there. **Move it back to its usual place** undoes it.
- **Rebuild the index** reads all your files again. Your folders and settings stay.
- **Delete all data** removes the index, your folders and settings, and the logs. Catchword starts again as new. Your own files are not touched.

If your index is in a folder that OneDrive, Dropbox, Google Drive or iCloud copies to the internet, Settings warns you.

### Diagnostics

Catchword keeps small logs on your computer, at most about 3 MB. They never hold document text or searches.

- **Detailed logs** also record file names and error details. Turn them on only while tracking down a problem.
- **Prepare a diagnostics report** shows you the report exactly as it will be saved. File names are left out unless you tick the box. **Save the report…** saves it as a file; nothing is sent. Share it only if you choose to.

### About and Privacy

The version, the licence (Apache 2.0), the licences of the parts Catchword uses, and the privacy promise.

## When something goes wrong

- **"The index was damaged, so it is being rebuilt"**: Catchword checks its index at every start. A damaged one is put aside and rebuilt from your files. Your folders and settings are kept. Press **OK** to close the message.
- **Catchword closed unexpectedly twice in a row**: it starts with indexing paused, in case a file is causing it. Resume when you are ready. If it happens again, rebuild the index, and prepare a diagnostics report to report the problem.
- **A result's file is "no longer where it was"**: it was moved, renamed or deleted. Choose **Scan now**.
- **A folder is "not reachable"**: connect its drive. Its files stay searchable meanwhile.
- **Scanned documents are not found:** they need text recognition, which is not available yet. They are listed in Library.
- **Files in OneDrive are not found:** files kept only in the cloud are not read, because reading them would download them. Make them available offline in OneDrive if you want them searched.

## Uninstalling

Uninstall Catchword from **Settings, Apps** in Windows. This removes the program, the index and the logs. Your settings are removed too if you tick **Also delete your settings**. Your own files are never touched.

If you moved the index to a folder of your own, uninstalling cannot find it there. Use **Delete all data** before you uninstall, or delete its `Catchword index` folder yourself.

## What Catchword can and cannot read yet

- **Reads:** PDFs that have a text layer (with page numbers), plain text (`.txt`) and Markdown (`.md`). Text in any common encoding, including older ones such as Windows-1252 and Windows-1256 (Arabic).
- **Not yet:** scanned documents without a text layer (text recognition comes in a later version), Word, Excel and other Office files (planned for version 0.2), and images.
- **Languages:** searching by meaning works across many languages. It was measured in English, German, French and Arabic.
