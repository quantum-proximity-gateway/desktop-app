import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Header from "./components/Header";
import "./App.css";
import Footer from "./components/Footer";
import { Button, Input, Text, Box, VStack, HStack, DrawerActionTrigger, DrawerBackdrop, DrawerBody, DrawerCloseTrigger, DrawerContent, DrawerFooter, DrawerHeader, DrawerRoot, DrawerTitle, DrawerTrigger } from "@chakra-ui/react";

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [messages, setMessages] = useState([{ sender: "", text: "" }]);
  const [chatID, setChatID] = useState("");
  const [preferences, setPreferences] = useState<AppConfig | null>(null);
  const [open, setOpen] = useState(false)

  async function listModels() {
    setModels(await invoke("list_models"));
  }

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

  type Commands = {
    windows: string;
    macos: string;
    gnome: string;
  };

  type DefaultValue = number | boolean | string;

  type Settings = {
    lower_bound?: number;
    upper_bound?: number;
    default: DefaultValue;
    commands: Commands;
  };

  type AppConfig = Record<string, Settings>;

  async function fetchPreferences() {
    try {
      const prefs = await invoke<AppConfig>("fetch_preferences");
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
      <Header />
      <Text fontSize="4xl" fontWeight="bold" textAlign="center" mt={8} mb={8}>Quantum Proximity Gateway - Preferences AI Agent</Text>
      <DrawerRoot open={open} onOpenChange={(e) => setOpen(e.open)}>
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
                <Text fontWeight="bold" mb={2}>Current Preferences:</Text>
                <VStack align="start">
                  {Object.entries(preferences).map(([key, settings], index) => (
                    <Box key={index} borderWidth="1px" borderRadius="md" p={4} width="100%">
                      <Text fontWeight="bold" mb={1}>{key}</Text>
                      <Text>Default: {settings.default.toString()}</Text>
                      {settings.lower_bound !== undefined && (
                        <Text>Lower Bound: {settings.lower_bound}</Text>
                      )}
                      {settings.upper_bound !== undefined && (
                        <Text>Upper Bound: {settings.upper_bound}</Text>
                      )}
                      <Text>Commands:</Text>
                      <VStack align="start" pl={4}>
                        <Text>Windows: {settings.commands.windows}</Text>
                        <Text>MacOS: {settings.commands.macos}</Text>
                        <Text>GNOME: {settings.commands.gnome}</Text>
                      </VStack>
                    </Box>
                  ))}
                </VStack>
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
          <DrawerCloseTrigger />
        </DrawerContent>
      </DrawerRoot>
      <VStack align="stretch" flex="1">
        <Box>
          <Text mt={3} mb={1}>Available models:</Text>
          <HStack mt={1}>
            {models.map((model, index) => (
              <Button key={index} onClick={() => selectModel(model)}>{model}</Button>
            ))}
          </HStack>
          <Text mt={1}>Selected model: {selectedModel}</Text>
        </Box>
        <Box className="container" mb={19}>
          <Text fontSize="lg">Example Prompts:</Text>
          <Text>“The text is too small, please make it bigger.”</Text>
          <Text>“Can you change the font style to ...?”</Text>
          <Text>“I need a larger cursor for better visibility.”</Text>
          <Text>“Please adjust the zoom for better readability.”</Text>
          <Text>“Could you disable animations please?”</Text>
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
        <Footer />
      </VStack>
    </Box>
  );
}

export default App;
