import React from "react";
import { Box, Button, HStack, Text } from "@chakra-ui/react";

const Header = () => {
  return (
<Box bg="blue.500" p={2} display="flex" justifyContent="space-between" alignItems="center">
<Text color="white" fontSize="lg" fontWeight="bold" textShadow="0 0 5px rgba(255, 255, 255, 0.7)" marginBottom="20px">Quantum Proximity Gateway Preferences AI Agent</Text>
        <HStack>
         <Button variant="outline" color="white">Home</Button>
         <Button variant="outline" color="white">Preferences</Button>
         <Button variant="outline" color="white">Devices</Button>
         <Button variant="outline" color="white">Profiles</Button>
        <Button variant="outline" colorScheme="whiteAlpha">User</Button>
      </HStack>
    </Box>
  );
};

export default Header;
