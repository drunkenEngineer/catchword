// Every piece of interface text, in one place, so it can be translated
// later (A11Y-4). Document text never goes here: it is shown as it is.

export const strings = {
  app: "Catchword",
  navigation: "Main navigation",
  destinations: { search: "Search", library: "Library", settings: "Settings" },

  chip: {
    noFolders: "No folders yet",
    starting: "Starting…",
    readingFiles: (done: number, total: number) => `Reading files: ${done} of ${total}`,
    meaning: (percent: number) => `Searchable by meaning: ${percent}%`,
    upToDate: "Up to date",
  },

  search: {
    box: "Search your documents",
    placeholder: "Describe what you are looking for, or type a word, a name or a number",
    noFoldersTitle: "Choose a folder to search",
    noFoldersText: "Catchword searches the folders you choose. Nothing leaves this computer.",
    addFolder: "Add a folder",
    examplesTitle: "Try, for example:",
    examples: ["the letter about my tax refund", "notice period in the lease", "invoice number"],
    summary: (files: number) => `${files.toLocaleString()} files ready to search.`,
    partial: "Indexing is under way, so results may be incomplete.",
    results: "Results",
    count: (files: number, ms: number) =>
      `${files} ${files === 1 ? "file" : "files"} in ${ms} ms`,
    nothing: (query: string) => `Nothing found for “${query}”.`,
    whyTitle: "Why might it be missing?",
    notIndexed: (count: number) =>
      `${count} ${count === 1 ? "file was" : "files were"} not indexed, for example scans with no text.`,
    stillIndexing: "Some files are still being indexed.",
    openLibrary: "Open Library",
    found: { words: "matched words", meaning: "matched meaning", both: "matched words and meaning" },
    copies: (n: number) => `and ${n} identical ${n === 1 ? "copy" : "copies"}`,
    preview: "Preview",
    open: "Open",
    reveal: "Show in folder",
    copy: "Copy passage",
    copied: "Copied",
  },

  library: {
    title: "Library",
    folders: "Folders",
    noFolders: "No folders yet.",
    addFolder: "Add a folder",
    scanNow: "Scan now",
    remove: "Remove",
    confirmRemove: "Remove this folder? Its text is removed from the index; your files are not touched.",
    confirm: "Remove folder",
    keep: "Keep",
    progress: "Progress",
    reading: (done: number, total: number) => `Reading files: ${done} of ${total}`,
    byWords: (files: number) => `Searchable by words: ${files.toLocaleString()} files`,
    byMeaning: (done: number, total: number) =>
      `Searchable by meaning: ${done.toLocaleString()} of ${total.toLocaleString()} passages`,
    meaningOff: (reason: string) => `Meaning search is off: ${reason}`,
    meaningLoading: "Meaning search is starting.",
    attention: "Needs attention",
    nothingNeedsAttention: "Every supported file was indexed.",
    failed: "failed",
    skipped: "skipped",
  },

  settings: {
    title: "Settings",
    privacyTitle: "Privacy",
    privacy:
      "Your files never leave this computer. Catchword sends no document text, search, file name or usage data anywhere, and has no account.",
    moreLater: "Folder exclusions, resource use and appearance settings arrive in a later version.",
  },
};
