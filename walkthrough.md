# Graphchan TypeScript Frontend Walkthrough

This document details the new TypeScript frontend for Graphchan, designed to replace the existing Rust frontend while maintaining full compatibility with the Rust backend.

## 1. Overview

The new frontend is built with **Vite + React + TypeScript**. It features a "slick but retro, minimal but high tech" aesthetic inspired by Neon Genesis Evangelion and Bloomberg terminals.

### Key Features
*   **Dashboard**: Real-time system health, network status, and recent activity.
*   **Thread Index**: Searchable list of threads with metadata.
*   **Thread View**:
    *   **List Mode**: Traditional linear post view.
    *   **Graph Mode**: Force-directed graph visualization of the thread structure (D3.js).
    *   **Timeline Mode**: Placeholder for future chronological projection.
*   **Posting**: Create threads and replies with file attachments (Images, Videos).
*   **Identity**: Manage local identity and P2P connections (Friend Codes).
*   **Media**: Inline rendering of images and videos.

## 2. Setup & Running

### Prerequisites
*   Node.js (v16+)
*   Running `graphchan_backend` (default: `http://localhost:8080`)

### Installation
```bash
cd graphchan_ts_frontend
npm install
```

### Development Server
To start the development server:
```bash
npm run dev
```
Access the app at `http://localhost:5173`.

### Production Build
To build for production:
```bash
npm run build
```
The artifacts will be in `dist/`.

## 3. Project Structure

*   `src/api/`: Strongly-typed API client using `axios`.
*   `src/components/`: Reusable UI components (`Layout`, `Post`, `ThreadGraph`).
*   `src/pages/`: Application pages (`Dashboard`, `ThreadList`, `ThreadView`, `Identity`).
*   `src/index.css`: Global retro/high-tech theme variables and styles.

## 4. Aesthetic & Design

The design uses a custom CSS system (no Tailwind) with:
*   **Colors**: Dark background (`#050505`), Neon Orange (`#ff9900`), and Cyan (`#00f3ff`) accents.
*   **Typography**: Monospace fonts (`Consolas`, `Courier New`) for a terminal feel.
*   **Visuals**: Scanline overlays, grid backgrounds, and high-contrast borders.

## 5. Verification

The project has been successfully built using `npm run build`.
*   **Linting**: All TypeScript errors and unused variables have been resolved.
*   **Routing**: `react-router-dom` handles navigation between views.
*   **API**: The client is configured to talk to `localhost:8080` by default.

## 6. Next Steps

*   Implement the **Timeline View** logic (currently a placeholder).
*   Add more sophisticated graph interactions (filtering, expansion).
*   Implement the **4chan Importer** UI (backend support required).
