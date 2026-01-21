# Manual Testing Checklist - Phase 9: Color System and Saved Searches

**Date**: 2026-01-21
**Feature**: Note Colors, Tag Colors, Color Search, and Saved Searches
**Dev Server**: http://localhost:5175/

## Test Environment Setup

1. Open dev server in browser: http://localhost:5175/
2. Create test account or use existing account
3. Ensure you have some test notes with various tags

## Test Cases

### 1. Note Color Assignment and Display

#### 1.1 Assign Color to Note
- [ ] Open a note in the editor
- [ ] Click the "More menu" (⋮) in editor toolbar
- [ ] Click "Set Colour"
- [ ] Select a color from the color picker modal
- [ ] **Expected**: Color picker closes, note shows selected color in note list
- [ ] **Expected**: Editor background changes to match selected color
- [ ] **Expected**: Toast notification shows "Colour set"

#### 1.2 Note Background Color in List
- [ ] Verify colored note shows background color in note list
- [ ] **Expected**: Note item has colored background
- [ ] **Expected**: Text remains readable (good contrast)
- [ ] **Expected**: Selected note still shows color (with blue left border)

#### 1.3 Remove Note Color
- [ ] Open a colored note
- [ ] Click "More menu" → "Set Colour"
- [ ] Click "No Colour" option
- [ ] **Expected**: Color removed from note
- [ ] **Expected**: Toast shows "Colour removed"
- [ ] **Expected**: Note list item returns to default background

### 2. Tag Color Configuration

#### 2.1 Assign Color to Tag (Settings)
- [ ] Open Settings (gear icon or Cmd+,)
- [ ] Navigate to "Colours" tab
- [ ] Scroll to "Tag Colours" section
- [ ] Click "Add Tag Colour"
- [ ] Select or type a tag name (should autocomplete existing tags)
- [ ] Select a color from dropdown
- [ ] **Expected**: Tag color appears in list

#### 2.2 Tag Color Display
- [ ] Create/view a note with the colored tag
- [ ] **Expected**: Tag pill shows assigned color background
- [ ] **Expected**: Tag color is independent of note color
- [ ] **Expected**: Multiple notes with same tag show same tag color

#### 2.3 Remove Tag Color
- [ ] In Settings → Colours → Tag Colours
- [ ] Click "Remove" next to a tag color assignment
- [ ] **Expected**: Tag color removed
- [ ] **Expected**: Tag pills return to default styling

### 3. Color Palette Customization

#### 3.1 Customize Color Palette
- [ ] Open Settings → Colours
- [ ] In "Colour Palette" section, modify light/dark hex values for a color
- [ ] Click outside the input to save
- [ ] **Expected**: Color changes immediately in preview swatch
- [ ] Switch to notes view
- [ ] **Expected**: Notes with that color show new color

#### 3.2 Light/Dark Theme Colors
- [ ] Assign a color to a note
- [ ] Toggle theme: Settings → General → Theme (Light/Dark/Auto)
- [ ] **Expected**: Note color changes appropriately for light vs dark mode
- [ ] **Expected**: Good contrast maintained in both modes

#### 3.3 Reset Palette to Defaults
- [ ] Customize some colors in palette
- [ ] Click "Reset to Defaults" button
- [ ] **Expected**: Confirmation modal appears
- [ ] Confirm reset
- [ ] **Expected**: All colors return to default values
- [ ] **Expected**: Notes show default colors

### 4. Color Search

#### 4.1 Search by Color (Both Note and Tag)
- [ ] Create notes with different colors
- [ ] Create tags with different colors
- [ ] Search: `color:red`
- [ ] **Expected**: Shows notes with red color OR notes with red-colored tags
- [ ] **Expected**: Search count shows correct number of matches

#### 4.2 Search Note Color Only
- [ ] Search: `color:red note`
- [ ] **Expected**: Shows only notes with red background color
- [ ] **Expected**: Excludes notes that only have red tags

#### 4.3 Search Tag Color Only
- [ ] Search: `color:blue tag`
- [ ] **Expected**: Shows only notes with blue-colored tags
- [ ] **Expected**: Excludes notes with blue background

#### 4.4 Plain Text Color Search
- [ ] Search: `red`
- [ ] **Expected**: Shows notes with "red" in content OR red color/tag
- [ ] **Expected**: Text search + color search combined

#### 4.5 Multiple Color Search
- [ ] Search: `color:red color:blue`
- [ ] **Expected**: Shows notes with red OR blue colors

### 5. Bulk Color Operations

#### 5.1 Multi-Select Notes
- [ ] Select multiple notes (Cmd+Click or Shift+Click)
- [ ] **Expected**: Bulk operations toolbar appears at bottom
- [ ] **Expected**: Selected count shown

#### 5.2 Bulk Assign Color
- [ ] With multiple notes selected, click "Set Colour"
- [ ] Select a color
- [ ] **Expected**: Color picker closes
- [ ] **Expected**: All selected notes change to selected color
- [ ] **Expected**: Toast shows "Colour set for N notes"

#### 5.3 Bulk Remove Color
- [ ] Select multiple colored notes
- [ ] Click "Set Colour" → "No Colour"
- [ ] **Expected**: All selected notes lose their color

### 6. Saved Searches - Basic Operations

#### 6.1 Create Saved Search
- [ ] Enter a search query (e.g., `#work color:red`)
- [ ] Click "Saved Searches" button (bookmark icon)
- [ ] Click "Save Current Search"
- [ ] Enter a name: "Red Work Items"
- [ ] Save
- [ ] **Expected**: Saved search appears in saved searches list
- [ ] **Expected**: Toast notification confirms save

#### 6.2 Apply Saved Search
- [ ] Click "Saved Searches" button
- [ ] Click on a saved search from the list
- [ ] **Expected**: Search query applied to search bar
- [ ] **Expected**: Notes filtered to match saved search
- [ ] **Expected**: Search count updates

#### 6.3 Edit Saved Search
- [ ] Click "Saved Searches" → Edit button (pencil icon) next to search
- [ ] Modify name or query
- [ ] Save changes
- [ ] **Expected**: Saved search updated
- [ ] **Expected**: Changes reflected immediately

#### 6.4 Delete Saved Search
- [ ] Click "Saved Searches" → Delete button (trash icon) next to search
- [ ] **Expected**: Confirmation modal appears
- [ ] Confirm deletion
- [ ] **Expected**: Saved search removed from list
- [ ] **Expected**: Toast notification confirms deletion

#### 6.5 Reorder Saved Searches
- [ ] Click "Saved Searches"
- [ ] Drag and drop saved searches to reorder
- [ ] **Expected**: Order changes persist
- [ ] **Expected**: New order maintained after closing and reopening panel

### 7. Sync Testing (Multi-Device)

#### 7.1 Saved Search Sync
- [ ] **Device A**: Create a saved search
- [ ] **Device A**: Wait for sync or trigger manual sync
- [ ] **Device B**: Refresh or wait for sync
- [ ] **Expected**: Saved search appears on Device B
- [ ] **Device B**: Apply the synced saved search
- [ ] **Expected**: Works correctly on Device B

#### 7.2 Note Color Sync
- [ ] **Device A**: Assign color to a note
- [ ] **Device A**: Sync
- [ ] **Device B**: Sync and verify
- [ ] **Expected**: Note shows same color on Device B

#### 7.3 Tag Color Sync
- [ ] **Device A**: Assign color to a tag in settings
- [ ] **Device A**: Sync
- [ ] **Device B**: Sync and verify
- [ ] **Expected**: Tag color setting synced
- [ ] **Expected**: Tags show same color on all notes on Device B

#### 7.4 Color Palette Sync
- [ ] **Device A**: Customize color palette
- [ ] **Device A**: Sync
- [ ] **Device B**: Sync and verify
- [ ] **Expected**: Custom palette colors appear on Device B
- [ ] **Expected**: Notes with those colors render correctly

#### 7.5 Conflict Resolution (Last-Write-Wins)
- [ ] **Device A**: Modify a saved search (offline)
- [ ] **Device B**: Modify same saved search differently (offline)
- [ ] Bring both online and sync
- [ ] **Expected**: Last modified version wins
- [ ] **Expected**: No data corruption or errors

### 8. Internationalization (i18n)

#### 8.1 Translation Coverage
- [ ] Switch language: Settings → General → Language
- [ ] Test each language (all 15):
  - [ ] en-GB (British English)
  - [ ] en-US (American English)
  - [ ] de (German)
  - [ ] es (Spanish)
  - [ ] fr (French)
  - [ ] it (Italian)
  - [ ] ja (Japanese)
  - [ ] ko (Korean)
  - [ ] nl (Dutch)
  - [ ] pl (Polish)
  - [ ] pt (Portuguese)
  - [ ] ru (Russian)
  - [ ] tr (Turkish)
  - [ ] zh (Chinese)
- [ ] **Expected**: All color-related UI text is translated
- [ ] **Expected**: No literal keys showing (e.g., "editor.setColor")
- [ ] **Expected**: Spelling correct for locale (colour vs color)

### 9. Edge Cases and Error Handling

#### 9.1 Long Saved Search Names
- [ ] Create saved search with very long name (100+ chars)
- [ ] **Expected**: Name truncates or wraps appropriately in UI
- [ ] **Expected**: Full name visible in edit mode

#### 9.2 Special Characters in Saved Search
- [ ] Create saved search with special chars: `#tag color:red -archived "quoted phrase"`
- [ ] **Expected**: Search works correctly
- [ ] **Expected**: Special characters preserved

#### 9.3 Deleted Tag Still Has Color
- [ ] Assign color to tag "temp"
- [ ] Delete all notes with tag "temp"
- [ ] **Expected**: Tag color remains in settings
- [ ] **Expected**: Can remove or keep for future use

#### 9.4 Color Picker with No Note Open
- [ ] Close all notes (note list only)
- [ ] Try to trigger color picker
- [ ] **Expected**: No error or appropriate message

#### 9.5 Many Saved Searches (100+)
- [ ] Create many saved searches (or import test data)
- [ ] **Expected**: UI remains performant
- [ ] **Expected**: Scrolling works smoothly
- [ ] **Expected**: Search/filter within saved searches if implemented

### 10. Performance Testing

#### 10.1 Large Dataset with Colors
- [ ] Create 1000+ notes with various colors
- [ ] **Expected**: Note list renders smoothly (virtual scrolling)
- [ ] **Expected**: Color search completes in <500ms
- [ ] **Expected**: No memory leaks over extended use

#### 10.2 Color Filter Performance
- [ ] With 1000+ notes, search `color:red`
- [ ] **Expected**: Results appear quickly
- [ ] **Expected**: UI remains responsive

### 11. Accessibility

#### 11.1 Keyboard Navigation
- [ ] Navigate color picker using keyboard only
- [ ] **Expected**: Can select colors with Tab + Enter
- [ ] **Expected**: Focus visible throughout

#### 11.2 Screen Reader Support
- [ ] Use screen reader (VoiceOver/NVDA)
- [ ] **Expected**: Color selections announced
- [ ] **Expected**: Saved searches list readable
- [ ] **Expected**: Proper ARIA labels present

#### 11.3 Color Contrast
- [ ] Verify all color combinations meet WCAG AA standards
- [ ] **Expected**: Text readable on all background colors
- [ ] **Expected**: Light/dark modes both accessible

## Test Results Summary

**Total Test Cases**: 60+
**Passed**: ___
**Failed**: ___
**Blocked**: ___
**Not Tested**: ___

## Issues Found

| ID | Severity | Description | Steps to Reproduce | Status |
|----|----------|-------------|-------------------|--------|
|    |          |             |                   |        |

## Notes

- All critical paths should be tested
- Focus on user-facing functionality
- Document any unexpected behavior
- Verify cross-browser compatibility (Chrome, Firefox, Safari, Edge)
- Test on mobile devices if applicable

## Sign-off

**Tester**: _______________
**Date**: _______________
**Status**: [ ] PASSED / [ ] FAILED / [ ] NEEDS REVIEW
