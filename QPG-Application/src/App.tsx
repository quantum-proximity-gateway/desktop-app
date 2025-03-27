import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";

import "./App.css";
import { Button, Input, Text, Box, HStack, Flex, Spinner, Code, Image } from "@chakra-ui/react";
import { Modal, ModalOverlay, ModalContent, ModalHeader, ModalFooter, ModalBody} from "@chakra-ui/modal";

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [messages, setMessages] = useState<MessageType[]>([{ sender: "", text: "" }]);
  const [chatID, setChatID] = useState("");
  const [preferences, setPreferences] = useState<string | null>(null);
  const [open, setOpen] = useState(false)
  const [showWelcome, setShowWelcome] = useState(true);
  const [isFading, setIsFading] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [pendingCommand, setPendingCommand] = useState<string | null>(null);
  const [online, setOnline] = useState<boolean>(true);

  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages, isLoading]);

  useEffect(() => {
    const timer = setTimeout(() => {
      setIsFading(true);
      setTimeout(() => {
        setShowWelcome(false);
      }, 1000); // match fadeOut duration
    }, 2000);
    return () => clearTimeout(timer);
  }, []);

  async function listModels() {
    setModels(await invoke("list_models"));
  }

  async function pingStatus() {
    setOnline(await invoke("check_encryption_client"));
  }

  type GenerateResult = {
    ollama_response: {
      model: string;
      created_at: string;
      message: ChatMessage;
      done: boolean;
    };
    command?: string;
  };


  type MessageType = {
    sender: string;
    text: string;
    timestamp?: Date;
  };
  
  type ChatMessage = {
    role: string;
    content: string;
  };

  async function fetchPreferences() {
    try {
      const preferences = await invoke<string>('fetch_full_json');
      setPreferences(preferences);
      setOpen(true);
    } catch (error) {
      console.error('Failed to fetch preferences:', error);
      throw error; // Re-throw the error to be handled by the caller
    }
  }

  async function generate() {
    if (isLoading || !prompt) {
      return;
    }
    if (!selectedModel) {
      alert("Please select a model first");
      return;
    }
    const userMessage = { sender: "user", text: prompt };
    setMessages([...messages, userMessage]);
    setPrompt("");
    setIsLoading(true);
    const result = await invoke<GenerateResult>("generate", { 
      request: { 
        model: selectedModel, 
        prompt, 
        chat_id: chatID 
      } 
    });
  
    const botMessage = { 
      sender: "bot", 
      text: result.ollama_response.message.content,
      timestamp: new Date()
    };
    
    setMessages([...messages, userMessage, botMessage]);
    
    if (result.command) {
      setPendingCommand(result.command);
    }
    
    setIsLoading(false);
  }

  function selectModel(model: string) {
    setSelectedModel(model);
    setChatID(model + Date()); // Date to differentiate when new chats with same model started
    setMessages([{ sender: "", text: "" }]);
    setIsLoading(false);
  }

  useEffect(() => {
    pingStatus();
    listModels();
  }, []);

    useEffect(() => {
	if (!showWelcome) {
	  emit("frontend-loaded", {})
	      .then(() => console.log("frontend-loaded event emitted"))
	      .catch((e) => console.log("Error emitting frontend-loaded event:", e));
	}
    }, [showWelcome]);

  return (
    <>
    <style>{`
    `}</style>

    
    {showWelcome && (
        <Flex
        direction="column"
        align="center"
        justify="center"
        minH="100vh"
        py={2}
        className={isFading ? 'fadeOut' : 'fadeIn'}
        fontFamily="IBM Plex Sans, sans-serif"
        >
        <Text fontSize="2xl" textAlign="center">IBM Proximity Agents</Text>
        </Flex>
      )}
      {!showWelcome && (
      <Box 
        className="App fadeIn" 
        p={6} 
        display="flex" 
        flexDirection="column" 
        height="100vh"
        bg="gray.50"
      >
        <Modal isOpen={!!pendingCommand} onClose={() => setPendingCommand(null)} isCentered>
          <ModalOverlay bg="rgba(0, 0, 0, 0.6)" backdropFilter="blur(10px)" />
          <ModalContent borderRadius="xl" boxShadow="lg">
            <ModalHeader fontWeight="bold" fontSize="4xl" textAlign="center">
              Confirm Command Execution
            </ModalHeader>
            <ModalBody display="flex" flexDirection="column" alignItems="center">
              <Text mb={2} textAlign="center">Would you like to execute this command?</Text>
              <Code p={2} my={2} display="block" width="100%" textAlign="center">
                {pendingCommand}
              </Code>
              <Text textAlign="center">This will modify your system settings.</Text>
            </ModalBody>
            <ModalFooter display="flex" justifyContent="center">
              <Button mr={3} onClick={() => setPendingCommand(null)}>
                Cancel
              </Button>
              <Button colorScheme="blue" onClick={async () => {
                if (pendingCommand) {
                  try {
                    await invoke("execute_command", { command: pendingCommand, update: true });
                  } catch (error) {
                    alert(`Error: ${error}`);
                  }
                  setPendingCommand(null);
                }
              }}>
                Execute
              </Button>
            </ModalFooter>
          </ModalContent>
        </Modal>
        
        <Flex 
          direction="row" 
          justify="space-between" 
          align="center" 
          mb={6} 
          pb={4}
          borderBottomWidth="1px"
          borderBottomColor="gray.200"
        >
          <Flex align="center" gap={3}>
            <Image src="/ibm.png" alt="IBM Logo" width={125} height={50}/>
            <Text 
              fontSize="xl" 
              fontWeight="bold"
              background="linear-gradient(to right, #2c3e50, #4286f4)"
              backgroundClip="text"
            >
              Proximity Agents
            </Text>
          </Flex>
        
        <HStack>
          <Button 
            colorScheme="blue" 
            variant="ghost" 
            size="sm"
            onClick={() => fetchPreferences()}
          >
            <span style={{marginRight: '8px'}}>⚙️</span>
            Preferences
          </Button>

          <Modal isOpen={open} onClose={() => setOpen(false)} isCentered size="xl">
            <ModalOverlay bg="rgba(0, 0, 0, 0.3)" backdropFilter="blur(10px)" />
            <ModalContent borderRadius="lg" shadow="xl">
              <ModalHeader borderBottomWidth="1px" borderColor="gray.200">
                <Text fontSize="xl" fontWeight="bold">Preferences</Text>
              </ModalHeader>
              <ModalBody py={6}>
              {preferences ? (
                  <>
                    <pre>
                      <Code>{JSON.stringify(JSON.parse(preferences), null, 2)}</Code>
                    </pre>
                  </>
                ) : (
                  <Flex justify="center" align="center" height="200px">
                    <Spinner size="lg" color="blue.500" mr={4} />
                    <Text>Loading preferences...</Text>
                  </Flex>
                )}
              </ModalBody>
              <ModalFooter borderTopWidth="1px" borderColor="gray.200">
                <Button colorScheme="blue" onClick={() => setOpen(false)}>
                  Close
                </Button>
              </ModalFooter>
            </ModalContent>
          </Modal>
                    
          <Button 
            colorScheme="blue" 
            variant="ghost" 
            size="sm"
          >
            <span style={{marginRight: '8px'}}>🔄</span>
            Switch Agent
          </Button>
        </HStack>
      </Flex>
        
        {!online && (
          <Box 
            bg="red.100" 
            p={4}
            textAlign="center"
            borderRadius="md"
            mb={4}
            display="flex"
            alignItems="center"
            justifyContent="center"
            gap={2}
          >
            <span style={{fontSize: "18px"}}>⚠️</span>
            <Text color="red.800" fontSize="md" fontWeight="medium">
              Server is offline, changes will not be saved.
            </Text>
          </Box>
        )}
        
        <Box mb={5}>
          <Text fontSize="md" fontWeight="medium" mb={3} color="gray.600" textAlign="center">Select a model to begin</Text>
          <Flex justifyContent="center" wrap="wrap" gap={2}>
            {models.sort().map((model, index) => (
              <Button 
                key={index} 
                onClick={() => selectModel(model)}
                className={`model-button ${selectedModel === model ? 'active' : ''}`}
                bg={selectedModel === model ? "transparent" : "white"}
                border="1px solid"
                borderColor={selectedModel === model ? "transparent" : "gray.200"}
                size="md"
                px={4}
              >
                {model}
              </Button>
            ))}
          </Flex>
        </Box>
        
        <Box 
          flex="1" 
          borderRadius="xl" 
          bg="white" 
          shadow="sm"
          border="1px" 
          borderColor="gray.200" 
          p={0}
          overflow="hidden"
          display="flex"
          flexDirection="column"
        >
          <Box 
            p={4} 
            overflowY="auto" 
            flex="1" 
            id="chat-messages"
            css={{
              "&::-webkit-scrollbar": {
                width: "8px",
              },
              "&::-webkit-scrollbar-track": {
                background: "#f1f1f1",
              },
              "&::-webkit-scrollbar-thumb": {
                background: "#c5c5c5",
                borderRadius: "4px",
              },
              "&::-webkit-scrollbar-thumb:hover": {
                background: "#a1a1a1",
              },
            }}
          >
            {messages.filter(msg => msg.sender !== "").map((message, index) => (
              <Box 
                key={index} 
                className={`chat-bubble ${message.sender === "user" ? "user-bubble" : "bot-bubble"}`}
              >
                {message.text}
                {message.timestamp && (
                  <Text 
                    fontSize="xs" 
                    opacity={0.7} 
                    textAlign="right" 
                    mt={1}
                  >
                    {message.timestamp.toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}
                  </Text>
                )}
              </Box>
            ))}
            
            {isLoading && (
              <Flex align="center" my={4} className="chat-bubble bot-bubble">
                <Spinner size="sm" color="blue.500" mr={3}/>
                <Text>Thinking...</Text>
              </Flex>
            )}
            <div ref={messagesEndRef} />
          </Box>
          
          <HStack 
            p={4}
            borderTopWidth="1px"
            borderTopColor="gray.200"
            bg="gray.50"
          >
            <Input 
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  generate();
                }
              }} 
              placeholder={selectedModel ? "Type your message..." : "Select a model to start chatting"} 
              value={prompt} 
              onChange={(e) => setPrompt(e.target.value)}
              variant="outline"
              bg="white"
              borderRadius="full"
              size="lg"
              disabled={!selectedModel || isLoading}
            />
            <Button 
              onClick={generate}
              disabled={!selectedModel || isLoading || !prompt}
              colorScheme="blue"
              borderRadius="full"
              size="lg"
              px={6}
            >
              Send
            </Button>
          </HStack>
        </Box>
      </Box>
    )}
    
    </>
  );
}

export default App;
