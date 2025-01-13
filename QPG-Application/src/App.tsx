import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Button, Input, Text, Box, VStack, HStack, DrawerActionTrigger, DrawerBackdrop, DrawerBody, DrawerCloseTrigger, DrawerContent, DrawerFooter, DrawerHeader, DrawerRoot, DrawerTitle, DrawerTrigger } from "@chakra-ui/react";

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [messages, setMessages] = useState([{ sender: "", text: "" }]);
  const [chatID, setChatID] = useState("");
  const [preferences, setPreferences] = useState<Preferences | null>(null);
  const [open, setOpen] = useState(false)

  async function listModels() {
    setModels(await invoke("list_models"));
  }

  type Message = {
    model: string;
    prompt: string;
  };

  type Response = {
    model: string;
    created_at: string;
    message: ChatMessage;
    done: boolean;
  };

  type ChatMessage = {
    role: string;
    content: string;
  };

  type Preferences = {
    zoom: number;
  };

  async function fetchPreferences() {
    try {
      const prefs = await invoke<Preferences>("fetch_preferences");
      setPreferences(prefs);
    } catch (error) {
      console.error("Failed to fetch preferences:", error);
    }
  }

  async function generate() {
    if (!selectedModel) {
      alert("Please select a model first");
      return;
    }
    const userMessage = { sender: "user", text: prompt };
    setMessages([...messages, userMessage]);
    setPrompt("");
    const response: Response = await invoke("generate", { request: { model: selectedModel, prompt, chat_id: chatID } });
    const botMessage = { sender: "bot", text: response.message.content };
    setMessages([...messages, userMessage, botMessage]);
  }

  function selectModel(model: string) {
    setSelectedModel(model);
    setChatID(model);
    setMessages([{ sender: "", text: "" }]);
  }

  useEffect(() => {
    listModels();
    fetchPreferences();
  }, []);

  return (
    <Box className="App" p={4} display="flex" flexDirection="column" height="100vh">
      <DrawerRoot open={open} onOpenChange={(e) => setOpen(e.open)}>
        <Text fontSize="2xl" textAlign="center" mb={4}>Quantum Proximity Gateway - Preferences AI Agent</Text>
        <DrawerBackdrop />
        <DrawerTrigger asChild>
          <Button variant="outline" size="sm">
            Preferences
          </Button>
        </DrawerTrigger>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>Preferences</DrawerTitle>
          </DrawerHeader>
          <DrawerBody>
            {preferences ? (
              <>
                <Text>Current Preferences:</Text>
                <Box mt={2}>
                  <Text>Zoom: {preferences.zoom}</Text>
                </Box>
              </>
            ) : (
              <Text>Loading preferences...</Text>
            )}
          </DrawerBody>
          <DrawerFooter>
            <DrawerActionTrigger asChild>
              <Button variant="outline">Close</Button>
            </DrawerActionTrigger>
          </DrawerFooter>
        </DrawerContent>
      </DrawerRoot>
      <VStack align="stretch" flex="1">
        <Box>
          <Text>Available models:</Text>
          <HStack mt={2}>
            {models.map((model, index) => (
              <Button key={index} onClick={() => selectModel(model)}>{model}</Button>
            ))}
          </HStack>
          <Text mt={2}>Selected model: {selectedModel}</Text>
        </Box>
        <Box border="1px" borderColor="gray.200" borderRadius="md" p={4} h="400px" overflowY="scroll" flex="1">
          {messages.map((message, index) => (
            <Box key={index} mb={2} textAlign={message.sender === "user" ? "right" : "left"}>
              <Text fontWeight={message.sender === "user" ? "bold" : "normal"}>{message.text}</Text>
            </Box>
          ))}
        </Box>
        <HStack mt={4}>
          <Input placeholder="Type your prompt:" value={prompt} onChange={(e) => setPrompt(e.target.value)} />
          <Button onClick={generate}>Send</Button>
        </HStack>
      </VStack>
    </Box>
  );
}

export default App;
