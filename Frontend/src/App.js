import React, { useState } from "react";
import "./Styles/page.css";

const AuroraAssistant = () => {
  const [output, setOutput] = useState("");
  const [loading, setLoading] = useState(false);
  const [showFileDropdown, setShowFileDropdown] = useState(false);
  const [showDangerDropdown, setShowDangerDropdown] = useState(false);
  const [showListDropdown, setShowListDropdown] = useState(false);
  const [showSystemDropdown, setShowSystemDropdown] = useState(false);
  const [showNavigationDropdown, setShowNavigationDropdown] = useState(false);
  const [showNanoDropdown, setShowNanoDropdown] = useState(false);
  const [showMemoryDropdown, setShowMemoryDropdown] = useState(false);
  const [searchTerm, setSearchTerm] = useState("");
  const [showHistory, setShowHistory] = useState(false);

  // List of commands that require arguments
  const commandsWithArgs = [
    "open_file",
    "create_file",
    "delete_file",
    "create_symlink",
    "print_file_content",
    "compile_code",
    "run_code",
  ];

  // List of commands that require two arguments
  const commandsWithTwoArgs = [
    "create_symlink"
  ];

  const handleCommand = async (command, argument = "") => {
    try {
      setLoading(true);
      const payload = argument ? { command: `${command} ${argument}` } : { command };
      const response = await fetch("http://127.0.0.1:8000/command", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        throw new Error(`Server responded with ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      setOutput(formatOutput(data.response));
    } catch (error) {
      setOutput(`Error: ${error.message || "Failed to connect to backend."}`);
    } finally {
      setLoading(false);
    }
  };

  const handleCommandWithArg = (cmd) => {
    if (commandsWithArgs.includes(cmd)) {
      // Special handling for commands that require two arguments
      if (commandsWithTwoArgs.includes(cmd)) {
        const sourceArg = prompt(`Enter source for "${cmd.replace(/_/g, " ")}" (e.g., filename):`);
        if (sourceArg === null) return; // User canceled first prompt
        
        const destArg = prompt(`Enter destination for "${cmd.replace(/_/g, " ")}" (e.g., new filename):`);
        if (destArg === null) return; // User canceled second prompt
        
        // Clean and combine both arguments
        const cleanedSourceArg = sourceArg.replace(/\bdot\b/g, ".").trim();
        const cleanedDestArg = destArg.replace(/\bdot\b/g, ".").trim();
        
        handleCommand(cmd, `${cleanedSourceArg} ${cleanedDestArg}`);
      } else {
        // Handle single argument commands
        const arg = prompt(`Enter argument for "${cmd.replace(/_/g, " ")}" (e.g., filename):`);
        if (arg === null) return; // User canceled prompt
        
        const cleanedArg = arg.replace(/\bdot\b/g, ".").trim();
        handleCommand(cmd, cleanedArg);
      }
    } else {
      // Handle commands with no arguments
      handleCommand(cmd);
    }
  };

  const formatOutput = (outputText) => {
    const lines = outputText.split("\n").filter((line) => line.trim() !== "");
    return (
      <ul className="output-list">
        {lines.map((line, idx) => (
          <li key={idx} className="output-line">{line}</li>
        ))}
      </ul>
    );
  };

  const startListening = () => {
    const recognition = new (window.SpeechRecognition || window.webkitSpeechRecognition)();
    recognition.lang = "en-US";
    recognition.start();

    recognition.onresult = (event) => {
      const voiceCommand = event.results[0][0].transcript.toLowerCase().trim();
      
      setSearchTerm(voiceCommand); // Display in search bar

      // List of all backend commands for matching
      const backendCommands = [
        "hello",
        "who_created_you",
        "current_day",
        "current_date",
        "current_time",
        "show_calendar",
        "disk_usage",
        "ps_command",
        "list_services",
        "list_users",
        "list_ports",
        "list_networks",
        "list_drives",
        "list_printers",
        "list_disks",
        "list_folders",
        "list_files",
        "open_file",
        "create_file",
        "delete_file",
        "create_symlink",
        "print_file_content",
        "search_file_or_folder",
        "cpu_usage",
        "df_command",
        "volume_up",
        "volume_down",
        "get_uptime",
        "memory_usage",
        "free_memory",
        "swap_memory",
        "go_home",
        "root_directory",
        "go_my_directory",
        "current_directory",
        "compile_code",
        "run_code",
        "shutdown",
        "restart",
        "logout",
        "hibernate",
        "sleep",
        "help",
        "exit",
      ];

      const matchedCommand = backendCommands.find((cmd) =>
        voiceCommand.includes(cmd.replace(/_/g, " "))
      );

      if (matchedCommand) {
        if (matchedCommand === "exit") {
          setOutput("Goodbye!"); // Display "Goodbye!" in UI
        
          fetch("http://localhost:3000/exit") // Send exit request to backend
            .then(() => {
              console.log("Backend exiting...");
              setTimeout(() => {
                window.close(); // Close frontend after backend stops
              }, 2000);
            })
            .catch((err) => {
              console.error("Error shutting down backend:", err);
              setOutput("Error: Could not exit properly.");
            });
        }
         else {
          // For voice commands, we'll need to extract arguments from the voice input
          const argument = voiceCommand
            .replace(matchedCommand.replace(/_/g, " "), "")
            .trim();
            
          // For commands that need two arguments, we'll need to prompt for the second one
          if (commandsWithTwoArgs.includes(matchedCommand) && argument) {
            const destArg = prompt(`Enter destination for "${matchedCommand.replace(/_/g, " ")}":`);
            if (destArg !== null) {
              const cleanedSourceArg = argument.replace(/\bdot\b/g, ".").trim();
              const cleanedDestArg = destArg.replace(/\bdot\b/g, ".").trim();
              handleCommand(matchedCommand, `${cleanedSourceArg} ${cleanedDestArg}`);
            }
          } else {
            handleCommand(matchedCommand, argument);
          }
        }
      } else {
        setOutput("Error: Command not recognized.");
      }
    };

    recognition.onerror = (event) => {
      console.error("Speech recognition error:", event.error);
      setOutput(`Speech recognition error: ${event.error}`);
    };
  };


  return (
    <div className="main-container">
      <nav className="navbar">
        <h1 className="navbar-title">Aurora Assistant</h1>
      </nav>

      <div className="content-container">
        <div className="left-section">
          <h2 className="section-header">Commands</h2>
          <div
            className="search-container"
            style={{ display: "flex", gap: "20px", alignItems: "center", height: "60px" }}
          >
            <input
              type="text"
              placeholder="Search commands..."
              className="search-bar"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
            <button className="voice-button" onClick={startListening}>
              🎤 Voice Command
            </button>
          </div>

          <div className="scrollable-commands">
            <div className="command-grid">
              {[
                "hello",
                "who_created_you",
                "current_day",
                "current_date",
                "help",
                "current_time",
                "show_calendar",
                "disk_usage",
                "ps_command",
              ].map((cmd, index) => (
                <button
                  key={index}
                  className="command-button"
                  onClick={() => handleCommandWithArg(cmd)}
                >
                  {cmd.replace(/_/g, " ")}
                </button>
              ))}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button"
                onClick={() => setShowListDropdown(!showListDropdown)}
              >
                List Commands ▼
              </button>
              {showListDropdown && (
                <div className="dropdown-content">
                  {[
                    "list_services",
                    "list_users",
                    "list_ports",
                    "list_networks",
                    "list_drives",
                    "list_printers",
                    "list_disks",
                    "list_folders",
                  ].map((cmd, index) => (
                    <button
                      key={index}
                      className="dropdown-item"
                      onClick={() => handleCommandWithArg(cmd)}
                    >
                      {cmd.replace(/_/g, " ")}
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button"
                onClick={() => setShowFileDropdown(!showFileDropdown)}
              >
                File Commands ▼
              </button>
              {showFileDropdown && (
                <div className="dropdown-content">
                  {[
                    "list_files",
                    "open_file",
                    "create_file",
                    "delete_file",
                    "create_symlink",
                    "print_file_content",
                  ].map((cmd, index) => (
                    <button
                      key={index}
                      className="dropdown-item"
                      onClick={() => handleCommandWithArg(cmd)}
                    >
                      {cmd.replace(/_/g, " ")}
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button"
                onClick={() => setShowSystemDropdown(!showSystemDropdown)}
              >
                System Commands ▼
              </button>
              {showSystemDropdown && (
                <div className="dropdown-content">
                  {["cpu_usage", "df_command", "volume_up", "volume_down", "get_uptime"].map(
                    (cmd, index) => (
                      <button
                        key={index}
                        className="dropdown-item"
                        onClick={() => handleCommandWithArg(cmd)}
                      >
                        {cmd.replace(/_/g, " ")}
                      </button>
                    )
                  )}
                </div>
              )}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button"
                onClick={() => setShowMemoryDropdown(!showMemoryDropdown)}
              >
                Memory Commands ▼
              </button>
              {showMemoryDropdown && (
                <div className="dropdown-content">
                  {["memory_usage", "free_memory", "swap_memory"].map((cmd, index) => (
                    <button
                      key={index}
                      className="dropdown-item"
                      onClick={() => handleCommandWithArg(cmd)}
                    >
                      {cmd.replace(/_/g, " ")}
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button"
                onClick={() => setShowNavigationDropdown(!showNavigationDropdown)}
              >
                Navigate Directory ▼
              </button>
              {showNavigationDropdown && (
                <div className="dropdown-content">
                  {["go_home", "root_directory", "go_my_directory", "current_directory"].map(
                    (cmd, index) => (
                      <button
                        key={index}
                        className="dropdown-item"
                        onClick={() => handleCommandWithArg(cmd)}
                      >
                        {cmd.replace(/_/g, " ")}
                      </button>
                    )
                  )}
                </div>
              )}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button"
                onClick={() => setShowNanoDropdown(!showNanoDropdown)}
              >
                Nano, Compile & Run ▼
              </button>
              {showNanoDropdown && (
                <div className="dropdown-content">
                  {["compile_code", "run_code"].map((cmd, index) => (
                    <button
                      key={index}
                      className="dropdown-item"
                      onClick={() => handleCommandWithArg(cmd)}
                    >
                      {cmd.replace(/_/g, " ")}
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="dropdown">
              <button
                className="dropdown-button danger"
                onClick={() => setShowDangerDropdown(!showDangerDropdown)}
              >
                Danger Commands ⚠ ▼
              </button>
              {showDangerDropdown && (
                <div className="dropdown-content">
                  {["shutdown", "restart", "logout", "hibernate", "sleep", "exit"].map(
                    (cmd, index) => (
                      <button
                        key={index}
                        className="dropdown-item danger"
                        onClick={() => handleCommandWithArg(cmd)}
                      >
                        {cmd.replace(/_/g, " ")}
                      </button>
                    )
                  )}
                </div>
              )}
            </div>
          </div>
        </div>

        <div className="right-section">
          <h2 className="section-header">Outputs</h2>
          <div className="scrollable-output">
            {loading ? "Loading..." : output || "Click a button to get output."}
          </div>
        </div>
      </div>
    </div>
  );
};

export default AuroraAssistant;