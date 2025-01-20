import React from "react";
import { Box, Text, Link, HStack } from "@chakra-ui/react";

const Footer = () => {
  return (
    <Box className="footer"> {/* Updated to use Footer.css */}
      <Text className="footer-text">© 2023 IBM Quantum Gateway. All rights reserved.</Text> {/* Updated to use Footer.css */}
      <HStack justifyContent="center" mt={2}>
        <Link className="footer-link" href="#">API Documentation</Link> {/* Updated to use Footer.css */}
        <Link className="footer-link" href="#">Contact Us</Link> {/* Updated to use Footer.css */}
      </HStack>
      <Text className="footer-text" mt={2}>Powered by IBM Quantum and Qiskit</Text> {/* Updated to use Footer.css */}
      <HStack justifyContent="center" mt={2}>
      </HStack>
    </Box>
  );
};

export default Footer;