import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { JobBoard } from "../target/types/job_board";

describe("job-board", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.JobBoard as Program<JobBoard>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
