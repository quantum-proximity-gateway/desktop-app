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
      <Text fontSize="2xl" textAlign="center" mb={4}>Quantum Proximity Gateway - Preferences AI Agent</Text>
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
          <Text>Available models:</Text>
          <HStack mt={2}>
            {models.map((model, index) => (
              <Button key={index} onClick={() => selectModel(model)}>{model}</Button>
            ))}
          </HStack>
          <Text mt={2}>Selected model: {selectedModel}</Text>
        </Box>
        <Box mb={4}>
          <Text fontSize="lg" textAlign="center">Example Prompts:</Text>
          <Text textAlign="center">“The text is too small, please make it bigger.”</Text>
          <Text textAlign="center">“Can you change the font style to ...?”</Text>
          <Text textAlign="center">“I need a larger cursor for better visibility.”</Text>
          <Text textAlign="center">“Please adjust the zoom for better readability.”</Text>
          <Text textAlign="center">“Could you disable animations please?”</Text>
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
