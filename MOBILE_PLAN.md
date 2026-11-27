# Splicedd Mobile - Implementation Plan

## Overview
Convert the desktop-only Tauri app to support mobile (Android/iOS) with download functionality instead of drag-and-drop.

## Phase 1: Tauri Mobile Setup
- [ ] Add mobile targets to `tauri.conf.json`
- [ ] Configure Android/iOS permissions (storage, internet)
- [ ] Add mobile icons and splash screens
- [ ] Update `Cargo.toml` with mobile-specific dependencies
- [ ] Configure `build.rs` for mobile targets

## Phase 2: Backend (Rust) - Download Commands
- [ ] Add `download_sample` command - downloads a single sample to app's data dir or user's Downloads
- [ ] Add `download_pack` command - downloads all samples in a pack (with progress)
- [ ] Add `get_download_dir` command - get platform-appropriate download directory
- [ ] Add `share_file` command - share downloaded file via system share sheet (mobile)
- [ ] Handle mobile filesystem differences (scoped storage on Android)

## Phase 3: Frontend - Mobile UI Adaptations
- [ ] Make layout responsive (stack filters vertically on mobile)
- [ ] Replace drag handlers with download buttons
- [ ] Add "Download Pack" button on pack filter banner
- [ ] Add download progress indicators (toast/notifications)
- [ ] Add pull-to-refresh for search results
- [ ] Touch-friendly controls (larger hit targets)

## Phase 4: SampleListEntry Component Changes
- [ ] Replace `onMouseDown` drag handler with download button
- [ ] Add download icon/button for each sample
- [ ] Show download progress per sample
- [ ] Add "Download Pack" button when viewing a pack

## Phase 5: Configuration & Build
- [ ] Update `tauri.conf.json` for mobile bundle settings
- [ ] Add mobile-specific CSP if needed
- [ ] Test Android build (`tauri android build`)
- [ ] Test iOS build (`tauri ios build` - requires macOS)

## Key Technical Details

### Mobile Download Strategy
- **Android**: Use `tauri-plugin-fs` to write to `Downloads` directory (requires `READ_EXTERNAL_STORAGE` / `WRITE_EXTERNAL_STORAGE` or scoped storage via `MediaStore`)
- **iOS**: Write to app's Documents directory, then use `UIDocumentInteractionController` or share sheet
- **Cross-platform**: Use `@tauri-apps/plugin-fs` with `BaseDirectory.Download` (Tauri 2.0+)

### Pack Download Flow
1. User clicks "Download Pack" on pack banner
2. Frontend fetches all samples in pack (paginated API calls)
3. Backend downloads each sample sequentially with progress updates
4. Frontend shows progress toast/notification
5. On completion, show success with option to open folder/share

### API Considerations
- Splice API returns paginated results (50 per page)
- Pack download needs to fetch all pages for `parent_asset_uuid`
- Rate limiting: add delays between requests
- Resume capability: track downloaded samples

## Files to Modify/Create

### New Files
- `src-tauri/src/mobile.rs` - Mobile-specific commands
- `src/ui/components/DownloadButton.tsx` - Reusable download button
- `src/ui/components/PackDownloadButton.tsx` - Pack download with progress
- `src/hooks/useDownload.ts` - Download logic hook

### Modified Files
- `src-tauri/tauri.conf.json` - Mobile config
- `src-tauri/Cargo.toml` - Mobile deps
- `src-tauri/src/main.rs` - Register mobile commands
- `src-tauri/src/files.rs` - Add download functions
- `src/native.ts` - Add download invocations
- `src/ui/App.tsx` - Responsive layout, pack download handler
- `src/ui/components/SampleListEntry.tsx` - Replace drag with download
- `src/splice/api.ts` - Add pack samples fetching
- `package.json` - Add mobile scripts

## Tauri Mobile Prerequisites
- Android: Android Studio, NDK, SDK configured
- iOS: Xcode (macOS only)
- Run `tauri mobile init` to scaffold mobile projects