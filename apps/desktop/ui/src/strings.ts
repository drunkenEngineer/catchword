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
    paused: "Paused",
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
    found: {
      words: "matched words",
      meaning: "matched meaning",
      both: "matched words and meaning",
      name: "matched the file or folder name",
    },
    modified: (secs: number) =>
      `changed ${new Date(secs * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })}`,
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
    scanning: "Scanning…",
    offline:
      "Not reachable: the drive is not connected, or the folder was moved. Its files stay searchable but cannot be opened.",
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
    pause: "Pause",
    resume: "Resume",
    pausedByYou: "Indexing is paused. Search works with what is indexed so far.",
    pausedLowDisk:
      "Indexing is paused: less than 1 GB is free on the drive that holds the index. Free some space, then resume.",
    pausedNewerIndex:
      "This index was made by a newer version of Catchword, so this version leaves it alone. Update Catchword, or rebuild the index here: all your files are read again.",
    rebuild: "Rebuild the index",
    attention: "Needs attention",
    nothingNeedsAttention: "Every supported file was indexed.",
    failed: "failed",
    skipped: "skipped",
    parked: "not tried again until you ask",
    retry: "Try again",
    retryHint: "Reads the files that failed again. Skipped files are read again when they change.",
  },

  settings: {
    title: "Settings",
    leaveOutTitle: "What to leave out",
    leaveOutText: "Catchword does not read what is left out, and removes it from the index.",
    excludedFolders: "Folders left out",
    noExcludedFolders: "No folders are left out.",
    excludeFolder: "Leave out a folder…",
    includeFolder: "Include again",
    patterns: "Names to leave out, one per line. * stands for any characters and ? for one.",
    save: "Save names",
    saved: "Saved. Indexing again.",
    restoreDefaults: "Restore the defaults",
    dataTitle: "Your data",
    dataPlace: (folder: string, size: string) => `The index is stored in ${folder} (${size}). It holds the text of your documents, so it stays on this computer.`,
    checkIndex: "Check the index",
    indexSound: "No damage found.",
    indexDamaged: "The index is damaged. Rebuild it to read your files again.",
    rebuild: "Rebuild the index",
    confirmRebuild:
      "Rebuild the index? Catchword reads all your files again; search works with what is done so far. Your folders and settings stay.",
    deleteAll: "Delete all data",
    confirmDeleteAll:
      "Delete the index and forget your folders and settings? Catchword starts again as new. Your own files are not touched.",
    keep: "Keep",
    indexingTitle: "Indexing",
    resourceMode: "How much of the processor indexing may use",
    modes: {
      light: ["Light", "One core. Slowest; best on battery or on an older computer."],
      balanced: ["Balanced", "Half the processor. The default."],
      fast: ["Fast", "All cores but one. Finishes soonest; the computer may get warm and loud."],
    } as Record<"light" | "balanced" | "fast", [string, string]>,
    modeNote: "Indexing always runs at low priority, so your other work comes first.",
    diagnosticsTitle: "Diagnostics",
    version: (version: string) => `Catchword ${version}`,
    detailedLogs: "Detailed logs: also record file names and error details. Turn on only while tracking down a problem.",
    logsNote: "Logs stay on this computer. They never hold document text or searches.",
    includePaths: "Include file and folder names in the report",
    prepareReport: "Prepare a diagnostics report",
    reportTitle: "The report, exactly as it will be saved",
    saveReport: "Save the report…",
    reportSaved: (name: string) => `Saved as ${name}. Nothing was sent; share it only if you choose to.`,
    privacyTitle: "Privacy",
    privacy:
      "Your files never leave this computer. Catchword sends no document text, search, file name or usage data anywhere, and has no account.",
    moreLater: "Resource use, updates and appearance settings arrive in a later version.",
  },

  welcome: {
    step: (step: number, of: number) => `Step ${step} of ${of}`,
    promiseTitle: "Your files never leave this computer",
    promise: "Catchword reads the folders you choose and builds a search index on this computer.",
    points: [
      "No document text, search, file name or usage data is sent anywhere.",
      "There is no account and no cloud service.",
      "Your files are never changed, moved or deleted.",
    ],
    continue: "Continue",
    foldersTitle: "Choose the folders to search",
    foldersText: "Pick the folders that hold your documents. You can change them later in Library.",
    addFolder: "Add a folder",
    addAnother: "Add another folder",
    start: "Start searching",
    skip: "Skip for now",
    leftOut: "System files and files that often hold passwords or keys are left out. You can change this in Settings.",
  },

  bytes: (bytes: number) =>
    bytes < 1024 * 1024
      ? `${Math.max(1, Math.round(bytes / 1024)).toLocaleString()} KB`
      : bytes < 1024 * 1024 * 1024
        ? `${(bytes / (1024 * 1024)).toLocaleString(undefined, { maximumFractionDigits: 1 })} MB`
        : `${(bytes / (1024 * 1024 * 1024)).toLocaleString(undefined, { maximumFractionDigits: 1 })} GB`,
};
