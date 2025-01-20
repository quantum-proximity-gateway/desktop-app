import React from "react";
import { Box, Button, HStack, Text } from "@chakra-ui/react";

const Header = () => {
  return (
<Box bg="blue.800" p={2} display="flex" justifyContent="space-between" alignItems="center">
<Text color="white" fontSize="lg" fontWeight="bold" textShadow="0 0 5px rgba(255, 255, 255, 0.7)" marginBottom="16px">IBM Quantum Proximity Gateway</Text>
        <HStack>
         <Button variant="outline" color="white" size="sm">Home</Button>
         <Button variant="outline" color="white" size="sm">Preferences</Button>
         <Button variant="outline" color="white" size="sm">Devices</Button>
         <Button variant="outline" color="white" size="sm">Profiles</Button>
        <Button variant="outline" colorScheme="whiteAlpha" size="sm">User</Button>
      </HStack>
    </Box>
  );
};

export default Header;
