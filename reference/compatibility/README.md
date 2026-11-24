# WorkWarrior and WebWarrior compatibility

[Download the generated comparison workbook](compatibility.xlsx) or open the
[comparison CSV](comparison.csv). Each row has a stable capability ID,
baseline parity, implementation references, and an alignment check.
The [baseline notes](read-me.csv) list the reviewed revisions and status definitions.

## Tracking where the products differ

**Product distinction** records which product leads or where each has different strengths.
**Parity intent** records whether to align a capability, track it independently, or leave
the decision open. A product lead does not imply a commitment to match it.

Append an entry to [Changes](changes.csv) for each notable improvement or divergence.
Record the capability ID, changed product, verification, and a pinned source link.
The comparison row describes the current state. Changes preserves its history.

## Updating the comparison

1. Edit the CSVs in this directory. Keep existing capability IDs and assign
   new IDs to newly scoped rows. Record implementation and verification
   separately and link the issue, PR, or validation evidence.
2. Pin evidence to a commit and a visible repository path. Link runtime,
   configuration, and development references in the implementation repository.
3. From the repository root, regenerate and validate the workbook.

   ```sh
   mach sheets build reference/compatibility
   mach sheets lint reference/compatibility
   ```

4. Commit the CSV changes and generated workbook together.
