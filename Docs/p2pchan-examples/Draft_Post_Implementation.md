# Draft Post Implementation Plan

## Overview
Implementation plan for draft post creation and submission in Graphchan, focusing on core functionality.

## Current Status
✅ Already Implemented:
- Basic draft window with title and content fields
- File attachment UI and basic handling
- Multiple parent post support
- Submit/Cancel buttons
- Reply button functionality
- Draft window appears properly centered with window frame
- Draft window shows all parent post IDs
- Click-to-add-parent functionality
- Parent ID removal with × button
- Database submission with multiple parents
- Post submission with titles (optional, defaults to " ")
- Basic author system (defaults to "Anonymous")

## Required Changes

### 1. ✅ Multiple Parent Support
- ✅ Update DraftPost struct to use Vec<i64> for parent_ids instead of Option<i64>
- ✅ Add parent ID display in UI
- ✅ Implement click-to-add-parent functionality in thread view

### 2. ✅ Basic Post Submission
- ✅ Convert DraftPost to Post structure
- ✅ Submit to database with required fields (title, content, author)
- ✅ Handle parent relationships in database
- ✅ Close window after successful submission

### 3. 🟡 File Attachments
- 🟡 Handle file uploads/attachments
- 🟡 Store files in appropriate directory
- 🟡 Create thumbnails
- 🟡 Update database with file metadata

### 4. 🟡 Future Enhancements
- 🟡 Implement proper user identification system
- 🟡 Add post validation (length limits, content filtering)
- 🟡 Support for rich text/markdown
- 🟡 Draft persistence (optional)
- 🟡 Error handling improvements

## Notes
- Keep everything in memory until submission
- No need for draft persistence
- Simple validation (non-empty content)
- Basic error handling for submission failures
- Default author to "Anonymous" until proper user system implemented
- Titles are optional, stored as " " if empty 