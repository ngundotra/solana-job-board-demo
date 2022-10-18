import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { JobBoard } from "../target/types/job_board";
import {
  PublicKey,
  Keypair
} from '@solana/web3.js';
import {
  getConcurrentMerkleTreeAccountSize,
  SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
  SPL_NOOP_PROGRAM_ID,
} from '@solana/spl-account-compression';

import {
  createAssociatedTokenAccount,
  createMint,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from '@solana/spl-token';

import { readFileSync } from 'fs';
import { SystemInstruction } from "@solana/web3.js";
import { SystemProgram } from "@solana/web3.js";
import { Connection } from "@solana/web3.js";
import { sendAndConfirmTransaction } from "@solana/web3.js";
import { Transaction } from "@solana/web3.js";
import { LAMPORTS_PER_SOL } from "@solana/web3.js";
import { BN } from "bn.js";

function keypairFromFile(filename: string): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(filename).toString())))
}

function getGlobalAuth(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("global_auth")], programId)[0];
}

function getBounty(sponsor: PublicKey, recipient: PublicKey, programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([
    Buffer.from("bounty"),
    sponsor.toBuffer(),
    recipient.toBuffer(),
  ], programId)[0]
}

describe("job-board", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.JobBoard as Program<JobBoard>;
  const provider = anchor.getProvider();
  const connection = provider.connection;

  const payerKp = keypairFromFile("/Users/noahgundotra/.config/solana/id.json");
  const payer = payerKp.publicKey

  const availableTreeKp = Keypair.generate();
  const availableTree = availableTreeKp.publicKey;

  const oracleTreeKp = Keypair.generate();
  const oracleTree = oracleTreeKp.publicKey;

  const globalAuth = getGlobalAuth(program.programId);
  console.log(`Global auth: ${globalAuth.toString()}`)

  it("Is initialized!", async () => {
    // Add your test here.
    const space = getConcurrentMerkleTreeAccountSize(20, 64);
    let txId = await program.methods
      .initializeGlobals()
      .accounts({
        availableTree,
        oracleTree,
        whitelistedKey: payer,
        globalAuth,
        splAccountCompression: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
        splNoop: SPL_NOOP_PROGRAM_ID,
      })
      .preInstructions([
        SystemProgram.createAccount({
          newAccountPubkey: oracleTree,
          fromPubkey: payer,
          programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
          space,
          lamports: await connection.getMinimumBalanceForRentExemption(space, "confirmed")
        }),
        SystemProgram.createAccount({
          newAccountPubkey: availableTree,
          fromPubkey: payer,
          programId: SPL_ACCOUNT_COMPRESSION_PROGRAM_ID,
          space,
          lamports: await connection.getMinimumBalanceForRentExemption(space, "confirmed")
        })
      ])
      .signers([payerKp, oracleTreeKp, availableTreeKp])
      .rpc();
    console.log("Your transaction signature", txId);

    const sponsorKp = Keypair.generate();
    const sponsor = sponsorKp.publicKey;
    await connection.requestAirdrop(sponsor, LAMPORTS_PER_SOL);

    // Setup the sponsor with mint instructions
    const tokenMint = await createMint(connection, payerKp, payer, payer, 9, undefined, { commitment: "confirmed" })
    const sponsorAta = await createAssociatedTokenAccount(connection, sponsorKp, tokenMint, sponsor, { commitment: "confirmed" })
    const mintToIx = createMintToInstruction(tokenMint, sponsorAta, payer, 10);
    let tx = new Transaction().add(mintToIx);
    txId = await sendAndConfirmTransaction(connection, tx, [payerKp])
    console.log("Your transaction signature", txId);

    const recipientKp = Keypair.generate();
    const recipient = recipientKp.publicKey;

    const bounty = getBounty(sponsor, recipient, program.programId);
    const bountyAta = await createAssociatedTokenAccount(connection, payerKp, tokenMint, recipient, {
      commitment: "confirmed"
    })

    let txInfo = await program
      .methods
      .createBounty(new BN(0), new BN(5))
      .accounts({
        tokenMint,
        sponsor: sponsor,
        sponsorTokenAccount: sponsorAta,
        recipient,
        bounty,
        bountyAta,
        splToken: TOKEN_PROGRAM_ID,
      })
      .signers([sponsorKp])
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", txInfo);

  });
});
