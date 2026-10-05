# What leaves your computer

**Nothing about your documents leaves your computer.** This version of Catchword makes no network requests at all. It has no account, no cloud service and no usage statistics.

This page says exactly what does and does not leave your computer, and how you can check it yourself.

## In detail

| What | Does it leave your computer? |
| --- | --- |
| Your documents, and the text Catchword reads from them | Never |
| The index, which holds that text and the "meaning" numbers made from it | Never. It stays in your user profile, or in a folder you chose |
| Your searches | Never |
| File and folder names | Never. A diagnostics report you prepare in Settings includes them only if you tick the box. Even then it is saved as a file on your computer, and nothing is sent |
| Usage statistics | Not collected |
| Crash reports | Written beside the app's logs, on your computer. Nothing is sent |
| Update check | Not in this version. The download from GitHub will later ask for one small file, to see whether a new version exists, and you can turn this off. GitHub would see your IP address and Catchword's version number. The Microsoft Store version never checks: Windows updates it |
| Installing from the Microsoft Store (when available) | Microsoft learns that you installed Catchword, as for any Store app |
| Installing from GitHub | GitHub sees the download, as for any download |

The embedding model, which lets Catchword search by meaning, comes inside the installer. Nothing is downloaded while you use the app.

## How Catchword keeps this promise

- **The engine has no network code.** The parts that read, index and search your documents are built without any library that can use the network. An automated check enforces this on every change (`scripts/check-no-network.sh`).
- **The window cannot contact anything.** Catchword's window shows pages that come from the app itself. Its security policy forbids them from connecting to any address.
- **Documents are read in a separate program** (`catchword-worker.exe`), which has no network code either.
- **The code is open source** (Apache-2.0). Once the repository is public, anyone can read it and check these points.

## What we checked

On 5 October 2026, on Windows 11, Catchword was started and watched for 20 seconds, sampling every 2 seconds. Neither the app nor any of its seven processes had a network connection open: `Catchword.exe` and the six Microsoft Edge WebView2 processes that draw its window.

Sampling can miss a connection that opens and closes between two samples. The firewall check below cannot miss one; it has not yet been run for this page, and you can run it yourself.

## Check it yourself

### With the firewall: block Catchword, and see that it still works

If Catchword needed the network, blocking it would break something. It does not.

1. Open the Start menu, type **firewall**, and open **Windows Defender Firewall with Advanced Security**.
2. Choose **Outbound Rules**, then **New Rule…** on the right.
3. Choose **Program**, then **Next**.
4. In **This program path**, enter `%LOCALAPPDATA%\Catchword\Catchword.exe`, then **Next**.
5. Choose **Block the connection**, then **Next**. Leave all three profiles ticked, then **Next**.
6. Name it **Block Catchword**, then **Finish**.
7. Repeat steps 2 to 6 for `%LOCALAPPDATA%\Catchword\catchword-worker.exe`, the program that reads your documents.
8. Use Catchword as usual: add a folder, let it index, and search. Everything should work, because nothing in Catchword needs the network. If something does not, please report it: that would be a bug in this promise.

To undo this, delete the two rules from **Outbound Rules**.

Catchword's window is drawn by Microsoft Edge WebView2 (`msedgewebview2.exe`). WebView2 is part of Windows and is shared by many apps, so blocking it could break other programs. Catchword's own pages are not allowed to contact any address, whatever the firewall says.

### Watching connections

For people comfortable with PowerShell, this lists any network connection held by Catchword or the programs it started. Run it while Catchword is open:

```powershell
$app = Get-Process Catchword
$all = @($app.Id) + (Get-CimInstance Win32_Process | Where-Object ParentProcessId -eq $app.Id).ProcessId
Get-NetTCPConnection | Where-Object { $all -contains $_.OwningProcess }
```

It should print nothing.

## Where your data is kept, and how to remove it

| What | Where |
| --- | --- |
| The program | `%LOCALAPPDATA%\Catchword` |
| The index (the text of your documents, for searching) | `%LOCALAPPDATA%\org.catchword.desktop\data`, unless you moved it in Settings |
| Your settings (your folders, what to leave out, your choices) | `%LOCALAPPDATA%\org.catchword.desktop\config` |
| Logs and crash reports (no document text, no searches) | `%LOCALAPPDATA%\org.catchword.desktop\logs` |

- **Settings, Delete all data** removes the index, wherever it is, together with your settings, logs and crash reports. Your own files are never touched.
- **Uninstalling** removes the program, the index and the logs. It removes your settings too if you tick the box.
- **If you moved the index** to a folder of your own, uninstalling cannot find it there. Use Delete all data before you uninstall, or delete its `Catchword index` folder yourself. Catchword warns you about this when you move it.
