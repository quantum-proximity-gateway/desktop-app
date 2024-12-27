import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Button, Input, Text, Box, VStack, HStack } from "@chakra-ui/react";

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [messages, setMessages] = useState([{ sender: "system", text: "Welcome! Please select a model and type your prompt." }]);
  const [chatID, setChatID] = useState("");
  
  async function listModels() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setModels(await invoke("list_models"));
  }

  type Message = {
    model: string;
    prompt: string;
  }

  type Response = {
    model: string,
    created_at: string,
    message: ChatMessage,
    done: boolean,
  }

  type ChatMessage = {
    role: String,
    content: string,
  }

  async function generate() {
    if (!selectedModel) {
      alert("Please select a model first");
      return;
    }
    const userMessage = { sender: "user", text: prompt };
    setMessages([...messages, userMessage]);
    setPrompt("");
    const response: Response = await invoke("generate", { request: { model: selectedModel, prompt, chatID } });
    const botMessage = { sender: "bot", text: response.message.content };
    setMessages([...messages, userMessage, botMessage]);
  }


  function selectModel(model: string) {
    setSelectedModel(model);
    setChatID(model);
    setMessages([{ sender: "system", text: "You're now chatting with: " + model }]);
  }

  useEffect(() => { // runs once when the component is mounted
    listModels();
  }, []);

  return (
    <Box className="App" p={4}>
      <Text fontSize="2xl" textAlign="center" mb={4}>Quantum Proximity Gateway - Preferences AI Agent</Text>
      <VStack align="stretch">
        <Box>
          <Text>Avaliable models:</Text>
          <HStack mt={2}>
            {models.map((model, index) => (
              <Button key={index} onClick={() => selectModel(model)}>{model}</Button>
            ))}
          </HStack>
          <Text mt={2}>Selected model: {selectedModel}</Text>
        </Box>
        <Box border="1px" borderColor="gray.200" borderRadius="md" p={4} h="400px" overflowY="scroll">
          {messages.map((message, index) => (
            <Box key={index} mb={2} textAlign={message.sender === "user" ? "right" : "left"}>
              <Text fontWeight={message.sender === "user" ? "bold" : "normal"}>{message.text}</Text>
            </Box>
          ))}
        </Box>
        <HStack>
          <Input placeholder="Type your prompt:" value={prompt} onChange={(e) => setPrompt(e.target.value)} />
          <Button onClick={generate}>Send</Button>
        </HStack>
      </VStack>
    </Box>

  );
}

export default App;
