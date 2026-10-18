# BDMV Remuxer

A powerful desktop application for remuxing Blu-ray Disc Movie (BDMV) folders into organized MKV files with proper episode numbering, track management, and batch processing.

![BDMV Remuxer](https://img.shields.io/badge/Tauri-1.6-blue) ![React](https://img.shields.io/badge/React-18-blue) ![Rust](https://img.shields.io/badge/Rust-1.75-orange)

## Features

### 📂 Smart File Detection
- **Auto-categorization** based on duration:
  - Episodes (>20 minutes)
  - Specials/Extras (>2 minutes, <20 minutes)
  - Openings/Endings (80-110 seconds, typically 1:31)
- **OP/ED Pairing** - Automatically detects and assigns OP1/ED1, OP2/ED2 based on duration patterns
- **Volume-based organization** - Correctly numbers episodes across multiple volumes

### 🎬 Track Management
- View all tracks (video, audio, subtitle) with codec, resolution, channels, and language
- Edit track names and languages
- **Track Layout Templates** - Define expected layouts per category
- **Mismatch Detection** - Alerts for files with extra/missing tracks (e.g., commentary)

### 📋 Queue System
- Add multiple BDMV projects to a processing queue
- Pause/Resume individual projects or the entire queue
- Progress tracking with current file display
- Custom output folder naming with `[Judas] Show Name` format

### 📁 Output Organization
- Episodes → `Episode/` subfolder
- Everything else (Specials, OPs, EDs) → `Extra/` subfolder
- Customizable folder names

## Prerequisites

Before building, ensure you have:

1. **Rust** (latest stable) - https://rustup.rs/
2. **Node.js** (v18+) - https://nodejs.org/
3. **MKVToolNix** - https://mkvtoolnix.download/
4. **FFmpeg/FFprobe** - https://ffmpeg.org/

### Platform-Specific Requirements

**Windows:**
```powershell
# Install Rust
winget install Rustlang.Rust.GNU

# Or use rustup
Invoke-WebRequest https://win.rustup.rs -OutFile rustup-init.exe
.\rustup-init.exe
```

**macOS:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies
brew install mkvtoolnix ffmpeg
```

**Linux (Ubuntu/Debian):**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev mkvtoolnix ffmpeg
```

## Installation

1. **Clone the repository:**
```bash
git clone https://github.com/your-username/bdmv-remuxer.git
cd bdmv-remuxer
```

2. **Install dependencies:**
```bash
npm install
```

3. **Run in development mode:**
```bash
npm run tauri dev
```

4. **Build for production:**
```bash
npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

### Updating the application icon

Edit `src-tauri/icons/icon.svg`, then run:

```bash
npm run icons:generate
npm run icons:verify
```

The generator rasterizes the SVG master and uses the Tauri CLI to refresh the PNG, ICO, and ICNS files referenced by `src-tauri/tauri.conf.json`.

## Usage

### 1. Load BDMV Volumes

Click **"Load Volume"** to select a single BDMV folder, or **"Load Multiple"** to select multiple volumes at once.

The application will:
- Scan the `BDMV/STREAM/` folder for `.m2ts` files
- Use FFprobe to get duration and track information
- Auto-categorize files based on duration
- Detect and pair OP/ED files
- Assign episode numbers across volumes

### 2. Review & Edit Classification

Each file shows:
- Duration
- File size
- Track count
- Current category (dropdown to change)
- Episode/OP/ED/Special number

Click the edit button to open the Track Editor and modify:
- Track languages
- Track names

### 3. Configure Track Templates

Set up expected track layouts for each category:
1. Click a template card (Episodes, OPs, EDs, etc.)
2. Define the expected tracks (video, audio, subtitle)
3. Set languages and names
4. Save the template

Files that don't match the template will be flagged.

### 4. Set Output Configuration

- **Output Name:** e.g., `[Judas] My Anime`
- **Output Path:** Where to save the remuxed files

### 5. Add to Queue & Process

1. Click **"Add to Queue"**
2. Repeat for additional BDMVs
3. Click **"Start"** to begin processing

The queue shows:
- Project status (Pending, Processing, Complete)
- Current file being processed
- Progress percentage

## Output Structure

```
Output Path/
└── [Judas] Show Name/
    ├── Episode/
    │   ├── [Judas] Show Name - E01.mkv
    │   ├── [Judas] Show Name - E02.mkv
    │   └── ...
    └── Extra/
        ├── [Judas] Show Name - OP1.mkv
        ├── [Judas] Show Name - ED1.mkv
        ├── [Judas] Show Name - SP01.mkv
        └── ...
```

## Configuration

Settings are stored in:
- **Windows:** `%APPDATA%\bdmv-remuxer\settings.json`
- **macOS:** `~/Library/Application Support/bdmv-remuxer/settings.json`
- **Linux:** `~/.config/bdmv-remuxer/settings.json`

### Detection Thresholds

| Setting | Default | Description |
|---------|---------|-------------|
| Min Episode Duration | 20 min | Files longer than this = Episodes |
| Min Extra Duration | 2 min | Files longer than this (but < episode) = Specials |
| OP/ED Min Duration | 80 sec | Lower bound for OP/ED detection |
| OP/ED Max Duration | 110 sec | Upper bound for OP/ED detection |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl/Cmd + O` | Load BDMV |
| `Ctrl/Cmd + Shift + O` | Load Multiple |
| `Ctrl/Cmd + S` | Save Settings |
| `Space` | Start/Pause Queue |

## Troubleshooting

### "mkvmerge not found"
Ensure MKVToolNix is installed and the path is correct in Settings.

### "FFprobe error"
Install FFmpeg and ensure `ffprobe` is in your PATH or set the path in Settings.

### Files not being detected
Check that:
- The folder structure is correct (`BDMV/STREAM/*.m2ts`)
- Files meet the minimum duration thresholds

## License

MIT License - See [LICENSE](LICENSE) for details.

## Credits

Built with:
- [Tauri](https://tauri.app/) - Desktop application framework
- [React](https://react.dev/) - UI framework
- [Zustand](https://github.com/pmndrs/zustand) - State management
- [Lucide](https://lucide.dev/) - Icons
- [MKVToolNix](https://mkvtoolnix.download/) - MKV muxing
- [FFmpeg](https://ffmpeg.org/) - Media analysis
