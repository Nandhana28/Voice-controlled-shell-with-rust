# Voice-controlled-shell-with-rust
A fast, efficient, and user-friendly voice-controlled command-line interface built with Rust.

A full-stack voice-controlled app using **Rust (Axum)** for the backend and **React (Node.js)** for the frontend.

---

##  Prerequisites

### Install Node.js and npm (for frontend)
- Download from [https://nodejs.org](https://nodejs.org)
- Verify:
  ```bash
  node -v
  npm -v
  ```

### Install Rust (for backend)
- Install via terminal:
   ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
- After installation, restart terminal and verify:
   ```bash
  rustc --version
  cargo --version
   ```

## Run the project
# 1. Start the Rust Backend
 ```bash
cd Backend
cargo run
 ```
# 2. Start the React Frontend
- If package.json is not present, initialize it by running npm init.
- Install dependencies using npm install.
 ```bash
cd Frontend
npm init
npm install react react-dom
npm install react-speech-recognition
npm start
```
