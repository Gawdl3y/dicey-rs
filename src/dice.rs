use std::{cmp, fmt};

use fastrand::Rng;

use crate::term::Describe;

/// `Dice` are a single set of one or more rollable dice of a specific number of sides,
/// along with a collection of modifiers to apply to any resulting rolls from them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dice {
	/// Number of dice to roll
	pub count: u8,

	/// Number of sides for each die
	pub sides: u8,

	/// Modifiers to apply to rolls from this set of dice
	pub modifiers: Vec<Modifier>,
}

impl Dice {
	/// Rolls the dice and applies all of its modifiers to the rolls using the default Rng.
	pub fn roll(&self) -> Result<Rolled, Error> {
		self.roll_using_rng(&mut Rng::new())
	}

	/// Rolls the dice and applies all of its modifiers to the rolls using the given Rng.
	pub fn roll_using_rng(&self, rng: &mut Rng) -> Result<Rolled, Error> {
		// Roll the dice!
		let mut rolls = Vec::with_capacity(self.count as usize);
		for _ in 0..self.count {
			rolls.push(self.roll_single_using_rng(rng));
		}

		// Apply all modifiers
		let mut rolls = Rolled { rolls, dice: self };
		for modifier in self.modifiers.iter() {
			modifier.apply_using_rng(&mut rolls, rng)?;
		}

		Ok(rolls)
	}

	/// Rolls a single die (with the same number of sides as the dice in this set)
	/// with no modifiers using the default Rng.
	#[must_use]
	pub fn roll_single(&self) -> DieRoll {
		DieRoll::new_rand(self.sides)
	}

	/// Rolls a single die (with the same number of sides as the dice in this set)
	/// with no modifiers using the given Rng.
	#[must_use]
	pub fn roll_single_using_rng(&self, rng: &mut Rng) -> DieRoll {
		DieRoll::new_rand_using_rng(self.sides, rng)
	}

	/// Creates a new set of dice matching this one but without any modifiers.
	#[must_use]
	pub fn plain(&self) -> Self {
		Self::new(self.count, self.sides)
	}

	/// Creates a new set of dice with a given count and number of sides.
	#[must_use]
	pub fn new(count: u8, sides: u8) -> Self {
		Self {
			count,
			sides,
			modifiers: Vec::new(),
		}
	}

	/// Creates a new dice builder.
	#[must_use]
	pub fn builder() -> Builder {
		Builder::default()
	}
}

impl Default for Dice {
	fn default() -> Self {
		Self::new(1, 20)
	}
}

impl fmt::Display for Dice {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(
			f,
			"{}d{}{}",
			self.count,
			self.sides,
			self.modifiers
				.iter()
				.map(|m| m.to_string())
				.collect::<Vec<_>>()
				.join("")
		)
	}
}

/// A `Modifier` is a routine that can be applied to a set of [Dice] to automatically manipulate resulting
/// [Rolled] dice sets from them as a part of their rolling process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Modifier {
	/// Rerolls (drops original and adds a newly-rolled die) dice that meet a condition.
	/// If the second parameter is `true`, the reroll is done recursively until the rerolled die no longer meets the
	/// condition.
	Reroll(Condition, bool),

	/// Explodes (keeps original and adds an additional newly-rolled die) dice that meet a condition.
	/// The default condition is being equal to the number of sides for the dice.
	/// If the second parameter is `true`, the explosion is done recursively for any additional rolls that also meet the
	/// condition.
	Explode(Option<Condition>, bool),

	/// Keeps only the highest x dice, dropping the rest
	KeepHigh(u8),

	/// Keeps only the lowest x dice, dropping the rest
	KeepLow(u8),
	//
	// /// Replace all dice lower than a given minimum value with the minimum
	// Min(u8),

	// /// Replace all dice higher than a given maximum value with the maximum
	// Max(u8),

	// /// Count the number of dice that meet or don't meet (second parameter `true` for meets, `false` for does not meet)
	// /// the given condition.
	// CountCond(Condition, bool),

	// /// Count the number of dice that are even (`true`) or odd (`false`)
	// CountParity(bool),

	// /// Subtract the number of dice that meet the given condition
	// SubCond(Condition),

	// /// Subtract the values of dice that meet the given condition
	// SubCondVal(Condition),

	// /// Subtract a value from the total
	// Margin(u8),
}

impl Modifier {
	/// Applies the modifier to a set of rolls using the default Rng where needed.
	pub fn apply<'rolled, 'modifier: 'rolled>(&'modifier self, rolls: &mut Rolled<'rolled>) -> Result<(), Error> {
		self.apply_using_rng(rolls, &mut Rng::new())
	}

	/// Applies the modifier to a set of rolls using a given Rng where needed.
	pub fn apply_using_rng<'rolled, 'modifier: 'rolled>(
		&'modifier self,
		rolled: &mut Rolled<'rolled>,
		rng: &mut Rng,
	) -> Result<(), Error> {
		match self {
			Self::Reroll(cond, recurse) => {
				// Prevent recursively rerolling dice that would result in infinite rerolls
				if *recurse {
					match cond {
						Condition::Eq(other) if *other == 1 && rolled.dice.sides == 1 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Condition::Gt(other) if *other == 0 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Condition::Gte(other) if *other <= 1 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Condition::Lt(other) if *other > rolled.dice.sides => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Condition::Lte(other) if *other >= rolled.dice.sides => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						_ => {}
					}
				}

				loop {
					// Determine which rolls qualify for reroll
					let mut to_reroll = rolled
						.rolls
						.iter_mut()
						.filter(|r| !r.is_dropped())
						.filter(|r| cond.check(r.val))
						.collect::<Vec<_>>();

					if to_reroll.is_empty() {
						break;
					}

					// Roll additional dice and drop the originals
					let mut rerolls = Vec::with_capacity(to_reroll.len());
					for roll in to_reroll.iter_mut() {
						let mut reroll = rolled.dice.roll_single_using_rng(rng);
						reroll.add(self);
						rerolls.push(reroll);
						roll.drop(self);
					}

					// Add the rerolls to the rolls
					rolled.rolls.append(&mut rerolls);

					if !*recurse {
						break;
					}
				}
			}

			Self::Explode(cond, recurse) => {
				// Prevent recursively exploding dice that would result in infinite explosions
				if *recurse {
					match cond {
						Some(Condition::Eq(other)) if *other == 1 && rolled.dice.sides == 1 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Some(Condition::Gt(other)) if *other == 0 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Some(Condition::Gte(other)) if *other <= 1 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Some(Condition::Lt(other)) if *other > rolled.dice.sides => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						Some(Condition::Lte(other)) if *other >= rolled.dice.sides => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						None if rolled.dice.sides == 1 => {
							return Err(Error::InfiniteRolls(rolled.dice.clone()));
						}
						_ => {}
					}
				}

				// Determine how many initial rolls qualify for explosion
				let mut to_explode = rolled
					.rolls
					.iter()
					.filter(|r| !r.is_dropped())
					.filter(|r| match cond {
						Some(cond) => cond.check(r.val),
						None => r.val == rolled.dice.sides,
					})
					.count();

				loop {
					// Roll additional dice
					let mut explosions = Vec::with_capacity(to_explode);
					for _ in 0..to_explode {
						let mut roll = rolled.dice.roll_single_using_rng(rng);
						roll.add(self);
						explosions.push(roll);
					}

					// Determine how many additional rolls qualify for explosion, then add the explosions to the rolls
					to_explode = recurse
						.then(|| {
							explosions
								.iter()
								.filter(|r| match cond {
									Some(cond) => cond.check(r.val),
									None => r.val == rolled.dice.sides,
								})
								.count()
						})
						.unwrap_or(0);
					rolled.rolls.append(&mut explosions);

					if to_explode == 0 {
						break;
					}
				}
			}

			Self::KeepHigh(count) => {
				let mut refs = rolled.rolls.iter_mut().filter(|r| !r.is_dropped()).collect::<Vec<_>>();
				refs.sort();
				refs.reverse();
				refs.iter_mut().skip(*count as usize).for_each(|roll| roll.drop(self));
			}

			Self::KeepLow(count) => {
				let mut refs = rolled.rolls.iter_mut().filter(|r| !r.is_dropped()).collect::<Vec<_>>();
				refs.sort();
				refs.iter_mut().skip(*count as usize).for_each(|roll| roll.drop(self));
			}
		};

		Ok(())
	}
}

impl fmt::Display for Modifier {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}{}",
			match self {
				Self::Reroll(_, recurse) => format!("r{}", recurse.then_some("r").unwrap_or("")),
				Self::Explode(_, recurse) => format!("x{}", recurse.then_some("").unwrap_or("o")),
				Self::KeepHigh(count) => format!("kh{}", if *count > 1 { count.to_string() } else { "".to_owned() }),
				Self::KeepLow(count) => format!("kl{}", if *count > 1 { count.to_string() } else { "".to_owned() }),
			},
			match self {
				Self::Reroll(cond, _) | Self::Explode(Some(cond), _) => cond.to_string(),
				Self::Explode(None, _) | Self::KeepHigh(..) | Self::KeepLow(..) => "".to_owned(),
			}
		)
	}
}

/// A `Condition` is a test that die values can be checked against.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Condition {
	Eq(u8),
	Gt(u8),
	Gte(u8),
	Lt(u8),
	Lte(u8),
}

impl Condition {
	/// Creates a condition from its corresponding symbol and a given value.
	pub fn from_symbol_and_val(symbol: &str, val: u8) -> Result<Self, Error> {
		Ok(match symbol {
			"=" => Self::Eq(val),
			">" => Self::Gt(val),
			">=" => Self::Gte(val),
			"<" => Self::Lt(val),
			"<=" => Self::Lte(val),
			_ => return Err(Error::UnknownCondition(symbol.to_owned())),
		})
	}

	/// Checks a value against the condition.
	pub fn check(&self, val: u8) -> bool {
		match self {
			Self::Eq(expected) => val == *expected,
			Self::Gt(expected) => val > *expected,
			Self::Gte(expected) => val >= *expected,
			Self::Lt(expected) => val < *expected,
			Self::Lte(expected) => val <= *expected,
		}
	}

	/// Gets the symbol that represents the condition.
	pub fn symbol(&self) -> &'static str {
		match self {
			Self::Eq(..) => "=",
			Self::Gt(..) => ">",
			Self::Gte(..) => ">=",
			Self::Lt(..) => "<",
			Self::Lte(..) => "<=",
		}
	}
}

impl fmt::Display for Condition {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}{}",
			self.symbol(),
			match self {
				Self::Eq(expected)
				| Self::Gt(expected)
				| Self::Gte(expected)
				| Self::Lt(expected)
				| Self::Lte(expected) => expected,
			}
		)
	}
}

/// A `DieRoll` is a single die resulting from rolling [Dice] and optionally applying modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DieRoll<'a> {
	/// Value that was rolled
	pub val: u8,

	/// Modifier that caused the addition of this die, if any
	pub added_by: Option<&'a Modifier>,

	/// Modifier that caused the drop of this die, if any
	pub dropped_by: Option<&'a Modifier>,
}

impl<'r> DieRoll<'r> {
	/// Marks this die roll as added by a given modifier.
	#[inline]
	pub fn add<'m: 'r>(&mut self, from: &'m Modifier) {
		self.added_by = Some(from);
	}

	/// Marks this die roll as dropped by a given modifier.
	#[inline]
	pub fn drop<'m: 'r>(&mut self, from: &'m Modifier) {
		self.dropped_by = Some(from);
	}

	/// Indicates whether this die roll was part of the original set (not added by a modifier).
	#[inline]
	pub fn is_original(&self) -> bool {
		self.added_by.is_none()
	}

	/// Indicates whether this die roll was been dropped by a modifier.
	#[inline]
	pub fn is_dropped(&self) -> bool {
		self.dropped_by.is_some()
	}

	/// Creates a new DieRoll with the given value.
	#[must_use]
	pub fn new(val: u8) -> Self {
		Self {
			val,
			added_by: None,
			dropped_by: None,
		}
	}

	/// Creates a new DieRoll with a random value using the default Rng.
	#[must_use]
	pub fn new_rand(max: u8) -> Self {
		let mut rng = Rng::new();
		Self::new_rand_using_rng(rng.u8(1..=max), &mut rng)
	}

	/// Creates a new DieRoll with a random value using the given Rng.
	#[must_use]
	pub fn new_rand_using_rng(max: u8, rng: &mut Rng) -> Self {
		Self::new(if max > 0 { rng.u8(1..=max) } else { 0 })
	}
}

impl PartialOrd for DieRoll<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for DieRoll<'_> {
	fn cmp(&self, other: &Self) -> cmp::Ordering {
		self.val.cmp(&other.val)
	}
}

impl fmt::Display for DieRoll<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}{}", self.val, if self.is_dropped() { " (d)" } else { "" })
	}
}

/// A representation of the result from rolling a single set of [Dice]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rolled<'a> {
	/// Each individual die roll that was made
	pub rolls: Vec<DieRoll<'a>>,

	/// Dice that were rolled to produce this
	pub dice: &'a Dice,
}

impl Rolled<'_> {
	/// Totals all roll values.
	pub fn total(&self) -> Result<u16, Error> {
		let mut sum: u16 = 0;

		// Sum all rolls that haven't been dropped
		for r in self.rolls.iter().filter(|r| !r.is_dropped()) {
			sum = sum.checked_add(r.val as u16).ok_or(Error::Overflow)?;
		}

		Ok(sum)
	}
}

impl Describe for Rolled<'_> {
	/// Builds a string of the dice expression the roll is from and all of the individual rolled dice.
	///
	/// Rolls that have been dropped are suffixed with `(d)`.
	///
	/// If `max_rolls`` is specified and there are more rolls than it, the output will be truncated and appended with
	/// "X more..." (where X is the remaining roll count past the max).
	///
	/// Example output: `3d6kh2[6, 2 (d), 5]`
	fn describe(&self, max_rolls: Option<usize>) -> String {
		let max_rolls = max_rolls.unwrap_or(usize::MAX);
		let total_rolls = self.rolls.len();
		let truncated_rolls = total_rolls.saturating_sub(max_rolls);

		format!(
			"{}[{}{}]",
			self.dice,
			self.rolls
				.iter()
				.take(max_rolls)
				.map(|r| r.to_string())
				.collect::<Vec<String>>()
				.join(", "),
			if truncated_rolls > 0 {
				format!(", {} more...", truncated_rolls)
			} else {
				"".to_owned()
			}
		)
	}
}

impl fmt::Display for Rolled<'_> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.describe(None))
	}
}

/// An error resulting from a dice operation
#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("integer overflow")]
	Overflow,

	#[error("{0} would result in infinite rolls")]
	InfiniteRolls(Dice),

	#[error("unknown condition symbol: {0}")]
	UnknownCondition(String),
}

/// Builds [Dice] with a fluent interface
#[derive(Debug, Clone, Default)]
pub struct Builder(Dice);

impl Builder {
	/// Sets the number of dice to roll.
	pub fn count(mut self, count: u8) -> Self {
		self.0.count = count;
		self
	}

	/// Sets the number of sides per die.
	pub fn sides(mut self, sides: u8) -> Self {
		self.0.sides = sides;
		self
	}

	/// Adds a reroll modifier to the dice.
	pub fn reroll(mut self, cond: Condition, recurse: bool) -> Self {
		self.0.modifiers.push(Modifier::Reroll(cond, recurse));
		self
	}

	/// Adds an exploding modifier to the dice.
	pub fn explode(mut self, cond: Option<Condition>, recurse: bool) -> Self {
		self.0.modifiers.push(Modifier::Explode(cond, recurse));
		self
	}

	/// Adds a keep highest modifier to the dice.
	pub fn keep_high(mut self, count: u8) -> Self {
		self.0.modifiers.push(Modifier::KeepHigh(count));
		self
	}

	/// Adds a keep lowest modifier to the dice.
	pub fn keep_low(mut self, count: u8) -> Self {
		self.0.modifiers.push(Modifier::KeepLow(count));
		self
	}

	/// Finalizes the dice.
	pub fn build(self) -> Dice {
		self.0
	}
}
