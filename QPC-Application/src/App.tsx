import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Button, Input, Text } from "@chakra-ui/react";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [models, setModels] = useState([]);

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }
  
  async function listModels() {
    setModels(await invoke("list_models"));
  
  }

  useEffect(() => { // runs once when the component is mounted
    listModels();
  }, []);

  return (
    <div className="App">
      <Text fontSize="2xl" textAlign="center" mb={4}>Quantum Proximity Gateway - Preferences AI Agent</Text> 
      <div style={{ display: "flex", flexDirection: "column", justifyContent: "center", alignItems: "center", height: "100vh", gap: "20px" }}>
        <Text>Avaliable models:</Text>
        <div style={{ marginTop: "20px" }}>
          {models.map((model, index) => (
            <div key={index} style={{ marginBottom: "10px" }}>
              <Button>{model}</Button>
            </div>
          ))}
        </div>
      </div>
  </div>

  );
}

export default App;
