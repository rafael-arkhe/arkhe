// =============================================================================
// BLOCO 470 v13 — DEPLOY EM SEPOLIA (O1)
// scripts/deploy-verifier.js
//
// Deploy do GovernanceVerifier (Groth16/BN254) em Sepolia com verificação
// automática no Etherscan.
//
// Uso:  PRIVATE_KEY=<key> ETHERSCAN_API_KEY=<key> \
//       SEPOLIA_RPC_URL=<url> npx hardhat run scripts/deploy-verifier.js --network sepolia
// =============================================================================
const hre = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
  console.log("🚀 Deploying Governance Verifier to Sepolia...");

  // Deploy do verifier
  const Verifier = await hre.ethers.getContractFactory("GovernanceVerifier");
  const verifier = await Verifier.deploy();
  await verifier.waitForDeployment();

  const address = await verifier.getAddress();
  console.log(`✅ Verifier deployed at: ${address}`);

  // Verificação no Etherscan
  console.log("🔍 Verifying on Etherscan...");
  try {
    await hre.run("verify:verify", {
      address: address,
      constructorArguments: [],
    });
    console.log("✅ Verified on Etherscan");
  } catch (e) {
    console.log("⚠️ Verification failed:", e.message);
  }

  // Salva o endereço para uso futuro
  const config = {
    network: "sepolia",
    verifierAddress: address,
    deployedAt: new Date().toISOString(),
    circuit: "governance_consensus",
    curve: "BN254",
    contract: "GovernanceVerifier",
  };

  const deploymentsDir = path.join(__dirname, "../deployments");
  fs.mkdirSync(deploymentsDir, { recursive: true });
  fs.writeFileSync(
    path.join(deploymentsDir, "sepolia.json"),
    JSON.stringify(config, null, 2)
  );

  console.log("📝 Deployment info saved to deployments/sepolia.json");
  return address;
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});