import React from "react";
import { Box, Text, Link, HStack } from "@chakra-ui/react";

const Footer = () => {
  return (
    <Box className="footer"> 
      <Text className="footer-text">© 2023 IBM Quantum Gateway. All rights reserved.</Text> 
      <HStack justifyContent="center" mt={2}>
        <Link className="footer-link" href="#">API Documentation</Link> 
        <Link className="footer-link" href="#">Contact Us</Link> 
      </HStack>
      <Text className="footer-text" mt={2}>Powered by IBM Quantum and Qiskit</Text> 
      <HStack justifyContent="center" mt={2}>
      </HStack>
    </Box>
  );
};

export default Footer;