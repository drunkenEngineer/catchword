// Every piece of interface text, in one place, so it can be translated
// later (A11Y-4). Document text never goes here: it is shown as it is.

export const strings = {
  app: "Catchword",
  /** Closes a notice from the start (the index or settings were damaged). */
  closeNotice: "OK",

  /** Checking for updates, in the download from GitHub (APP-1, APP-2). */
  updates: {
    askTitle: "Check for new versions?",
    ask: "Once a day, Catchword can ask GitHub, where it is published, whether a newer version exists. GitHub sees your computer's internet address, and nothing else: nothing about your documents, searches or files is sent. Nothing is installed without your OK.",
    yes: "Check once a day",
    no: "Don't check",
    available: (version: string) => `Catchword ${version} is available.`,
    install: "Install and restart",
    later: "Later",
    installing: "Downloading and installing. Catchword starts again when it is done.",
  },
  navigation: "Main navigation",
  destinations: { search: "Search", library: "Library", settings: "Settings" },

  chip: {
    noFolders: "No folders yet",
    starting: "Starting…",
    readingFiles: (done: number, total: number) => `Reading files: ${done} of ${total}`,
    readingStep: (percent: number) => `Reading files: ${percent}%`,
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
    filterFolder: "Folder",
    allFolders: "All folders",
    filterKind: "Kind",
    allKinds: "All kinds",
    kinds: { pdf: "PDF", text: "Text and Markdown" } as Record<"pdf" | "text", string>,
    tip: "Put words in quotes to find them exactly as written, for example “notice period”.",
    examples: ["the letter about my tax refund", "notice period in the lease", "invoice number"],
    summary: (files: number) => `${files.toLocaleString()} files ready to search.`,
    /** While indexing runs (spec section 8, states). An index without a
     * finished scan is new or being rebuilt: its results grow as files are read. */
    firstReading: (done: number, total: number) =>
      `Reading your files: ${done.toLocaleString()} of ${total.toLocaleString()}. Results appear as each file is read.`,
    checking: (done: number, total: number) =>
      `Checking your files for changes: ${done.toLocaleString()} of ${total.toLocaleString()}. Results may be incomplete.`,
    embedding: (done: number, total: number) =>
      `Searchable by meaning: ${done.toLocaleString()} of ${total.toLocaleString()} passages. Results may be incomplete.`,
    searching: "Searching…",
    results: "Results",
    count: (files: number, ms: number) =>
      `${files} ${files === 1 ? "file" : "files"} in ${ms} ms`,
    nothing: (query: string) => `Nothing found for “${query}”.`,
    whyTitle: "Why might it be missing?",
    /** Why files are not searchable, by cause, with how many (spec section 8). */
    causes: {
      "needs-ocr": (n: number) =>
        `${n} ${n === 1 ? "file is a scan" : "files are scans"} with no text layer, not searchable until text recognition arrives.`,
      encrypted: (n: number) => `${n} ${n === 1 ? "file is" : "files are"} protected by a password.`,
      "cloud-only": (n: number) =>
        `${n} ${n === 1 ? "file is" : "files are"} kept only in the cloud, so ${n === 1 ? "it was" : "they were"} not read.`,
      "too-large": (n: number) =>
        `${n} ${n === 1 ? "file is" : "files are"} over the size or page limit (see Settings).`,
      "cannot-open": (n: number) => `${n} ${n === 1 ? "file" : "files"} could not be opened.`,
      failed: (n: number) => `${n} ${n === 1 ? "file" : "files"} could not be read.`,
    } as Record<"needs-ocr" | "encrypted" | "cloud-only" | "too-large" | "cannot-open" | "failed", (n: number) => string>,
    filtered: "Only part of your files was searched, because of the filters above.",
    clearFilters: "Search everything",
    filterChanged: "Changed",
    anyTime: "Any time",
    changed: { pastWeek: "In the past week", pastMonth: "In the past month", pastYear: "In the past year" },
    missing: (name: string) =>
      `“${name}” is no longer where it was: it was moved, renamed or deleted since the last scan.`,
    scanNow: "Scan now",
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
    copyPath: "Copy path",
    /** A file's passages folded to its best one (Left folds, Right unfolds). */
    folded: (n: number) => `${n} more ${n === 1 ? "passage" : "passages"}`,
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
    pace: (perSecond: number, unit: "files" | "passages") =>
      `${perSecond < 10 ? perSecond.toFixed(1) : Math.round(perSecond).toLocaleString()} ${unit} a second`,
    left: (seconds: number) =>
      seconds < 60
        ? "less than a minute left"
        : seconds < 3600
          ? `about ${Math.round(seconds / 60)} ${Math.round(seconds / 60) === 1 ? "minute" : "minutes"} left`
          : `about ${Math.floor(seconds / 3600)} h ${Math.round((seconds % 3600) / 60)} min left`,
    lastScan: (secs: number) =>
      `Last scan: ${new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}`,
    byWords: (files: number) => `Searchable by words: ${files.toLocaleString()} files`,
    byMeaning: (done: number, total: number) =>
      `Searchable by meaning: ${done.toLocaleString()} of ${total.toLocaleString()} passages`,
    meaningOff: (reason: string) => `Meaning search is off: ${reason}`,
    meaningLoading: "Meaning search is starting.",
    pause: "Pause",
    resume: "Resume",
    pausedByYou: "Indexing is paused. Search works with what is indexed so far.",
    pausedBattery:
      "Indexing is paused while the computer runs on battery. It carries on when you plug it in, or resume it now.",
    pausedIndexAway:
      "The index is kept in a folder that cannot be reached now: is its drive connected? Connect it, then resume. Or move the index back to its usual place in Settings.",
    pausedLowDisk:
      "Indexing is paused: less than 1 GB is free on the drive that holds the index. Free some space, then resume.",
    pausedNewerIndex:
      "This index was made by a newer version of Catchword, so this version leaves it alone. Update Catchword, or rebuild the index here: all your files are read again.",
    pausedSafeMode:
      "Catchword closed unexpectedly twice in a row, so indexing is paused. Resume when you are ready. If it happens again, rebuild the index, and prepare a diagnostics report in Settings to report the problem.",
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
    moveIndex: "Move the index…",
    moveBack: "Move it back to its usual place",
    move: "Move",
    moving: "Moving the index…",
    moved: "The index was moved.",
    /** The warning before a move (APP-7, PRIV-7): the owner's choice, 5 October 2026. */
    confirmMove: (path: string, syncedBy: string | null) =>
      `Move the index to ${path}? Uninstalling Catchword will not remove it from there: before you uninstall, ` +
      `use Delete all data, or delete that folder yourself. If its drive is not connected, search and indexing wait for it.` +
      (syncedBy
        ? ` Warning: ${syncedBy} copies that folder to the internet, and the index holds the text of your documents.`
        : ""),
    confirmMoveBack: "Move the index back to its usual place, in your user profile?",
    indexSound: "No damage found.",
    indexDamaged: "The index is damaged. Rebuild it to read your files again.",
    rebuild: "Rebuild the index",
    confirmRebuild:
      "Rebuild the index? Catchword reads all your files again; search works with what is done so far. Your folders and settings stay.",
    syncedWarning: (service: string) =>
      `Warning: the index is in a folder that ${service} copies to the internet. It holds the text of your documents. Stop ${service} from syncing this folder.`,
    deleteAll: "Delete all data",
    confirmDeleteAll:
      "Delete the index and forget your folders and settings? Catchword starts again as new. Your own files are not touched.",
    keep: "Keep",
    appearanceTitle: "Appearance",
    theme: "Colours",
    themes: { system: "As Windows is set", light: "Light", dark: "Dark" } as Record<"system" | "light" | "dark", string>,
    textSize: "Text size",
    textSizes: { normal: "Normal", large: "Large (115%)", larger: "Larger (130%)" } as Record<
      "normal" | "large" | "larger",
      string
    >,
    indexingTitle: "Indexing",
    resourceMode: "How much of the processor indexing may use",
    modes: {
      light: ["Light", "One core. Slowest; best on battery or on an older computer."],
      balanced: ["Balanced", "Half the processor. The default."],
      fast: ["Fast", "All cores but one. Finishes soonest; the computer may get warm and loud."],
    } as Record<"light" | "balanced" | "fast", [string, string]>,
    modeNote: "Indexing always runs at low priority, so your other work comes first.",
    pauseOnBattery: "Pause indexing while the computer runs on battery. It carries on when you plug it in.",
    maxFileMb: "Largest file to read, in MB",
    maxPages: "Most pages to read from a PDF",
    saveLimits: "Save limits",
    limitsSaved: "Saved. Files skipped as too large are read again if they now fit.",
    diagnosticsTitle: "Diagnostics",
    version: (version: string) => `Catchword ${version}`,
    detailedLogs: "Detailed logs: also record file names and error details. Turn on only while tracking down a problem.",
    logsNote: "Logs stay on this computer. They never hold document text or searches.",
    includePaths: "Include file and folder names in the report",
    prepareReport: "Prepare a diagnostics report",
    reportTitle: "The report, exactly as it will be saved",
    saveReport: "Save the report…",
    reportSaved: (name: string) => `Saved as ${name}. Nothing was sent; share it only if you choose to.`,
    aboutTitle: "About",
    licence: "Catchword is free software under the Apache License 2.0.",
    showNotices: "Show the licences of the parts Catchword uses",
    noticesTitle: "Licences of the parts Catchword uses",
    noNotices: "The licences are added when the app is packaged (node scripts/notices.mjs).",
    privacyTitle: "Privacy",
    privacy:
      "Your files never leave this computer. Catchword sends no document text, search, file name or usage data anywhere, and has no account.",
    /** The network activity statement (spec section 8, Settings; docs/user/network.md). */
    network:
      "This build makes no network requests at all. The download from Microsoft Store is updated by Windows.",
    networkWithUpdates:
      "The only network request Catchword makes is the check for new versions below, and only if you turn it on.",
    updatesTitle: "Updates",
    updatesCheck: "Check for new versions once a day. GitHub sees your computer's internet address, and nothing else.",
    checkNow: "Check now",
    checking: "Checking…",
    upToDate: "You have the newest version.",
    lastUpdateCheck: (secs: number) =>
      `Last checked: ${new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}`,
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
    next: "Next",
    updatesLater: "You can change this at any time in Settings.",
    leftOut: "System files and files that often hold passwords or keys are left out. You can change this in Settings.",
  },

  bytes: (bytes: number) =>
    bytes < 1024 * 1024
      ? `${Math.max(1, Math.round(bytes / 1024)).toLocaleString()} KB`
      : bytes < 1024 * 1024 * 1024
        ? `${(bytes / (1024 * 1024)).toLocaleString(undefined, { maximumFractionDigits: 1 })} MB`
        : `${(bytes / (1024 * 1024 * 1024)).toLocaleString(undefined, { maximumFractionDigits: 1 })} GB`,
};
